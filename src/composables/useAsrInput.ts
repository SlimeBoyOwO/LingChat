import { ref, computed, shallowRef, watch } from 'vue'
import { useRoute, type RouteLocationNormalizedLoaded } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import { useUIStore } from '@/stores/modules/ui/ui'
import { useAsrStore } from '@/stores/modules/settings/asr'
import { useGameStore } from '@/stores/modules/game'
import {
  asrStartListening,
  asrStopListening,
  asrRecognizeWav,
  asrCancel,
  asrVadProcessChunk,
  asrRegisterHotkey,
  asrUnregisterHotkey,
  type AsrSource,
  type AsrResult,
  type VadEvent,
} from '@/api/services/asr'
import { pcmToWavPcm16 } from '@/utils/asrAudio'

/**
 * 统一 ASR 输入入口：三种触发源共用同一会话生命周期。
 *
 * 三种触发源：
 * - Button: GameDialog.vue 的 mic 按钮
 * - Hotkey: useGlobalHotkey.ts 注册的全局快捷键（App.vue 挂载一次）
 * - Auto: asrStore.settings.auto_listen=true 时由能量监测触发
 *
 * 窗口活跃门控：仅当 chatActive=true（/chat 路由 + 设置抽屉未开）时启用。
 * 失败降级：mic 不可用时 fail-open（不抛错到用户），退化为手动按钮 + 不录。
 *
 * ── 单例设计 ──────────────────────────────────────────────
 * 状态全部在模块级（非函数内）：App.vue 的 hotkey 实例与 GameDialog 的
 * mic 实例共享同一会话。若状态放在函数内，两实例各自持有 recorder/phase，
 * hotkey 录音时 GameDialog 的 mic 按钮看不到状态、互不感知。
 *
 * ── 采集链路（spec §3.1）─────────────────────────────────
 * 16kHz AudioContext + ScriptProcessor 直接拿 f32 PCM（不经过
 * MediaRecorder webm 编码），停止时合成 16k mono PCM16 WAV 送去识别。
 * auto 模式额外把每 512 samples（30ms）喂 asrVadProcessChunk，
 * 由后端 Silero VAD 做端点检测（turn_candidate → 一轮说话结束）。
 *
 * 队列设计说明：项目里没有专门的 useChatStore（聊天状态由 useGameStore.currentStatus
 * 体现：'input' = 空闲可输入，'thinking'/'responding'/'presenting' = 生成中）。
 * 因此用 gameStore 顶一个非类型化字段 pendingAsrQueue 做兜底，作为 ASR→chat 的
 * 跨组件排队通道（GameDialog 在 currentStatus 转回 'input' 时 flush）。
 */

// ── 模块级单例状态 ──────────────────────────────────────────
const phase = ref<'idle' | 'recording' | 'recognizing'>('idle')
const activeSource = shallowRef<AsrSource | null>(null)

/** 本次录音累积的 f32 PCM（16kHz mono） */
let pcmBuffer: number[] = []
/** 待喂 VAD 的积累块（凑满 512 samples = 30ms 才发） */
let vadPending: number[] = []
let stream: MediaStream | null = null
let audioCtx: AudioContext | null = null
let processor: ScriptProcessorNode | null = null
let energyMon: { ctx: AudioContext; raf: number; stream: MediaStream } | null = null
/** auto 触发去重：能量触发后不再重复触发，直到本轮会话结束 */
let autoTriggered = false
/** 惰性依赖（首次 useAsrInput() 调用时初始化） */
let route: RouteLocationNormalizedLoaded | null = null
let uiStore: ReturnType<typeof useUIStore> | null = null
let asrStore: ReturnType<typeof useAsrStore> | null = null
let gameStore: ReturnType<typeof useGameStore> | null = null

// 关键修正：spec §3.0 用 showSettings，不是 settingsOpen
const chatActive = computed(() => {
  if (!route || !uiStore) return false
  return route.path === '/chat' && !uiStore.showSettings
})

/** 生成中（非 input 即视为 busy，用于 auto_send 降级 queue） */
function isChatBusy(): boolean {
  return !!gameStore && gameStore.currentStatus !== 'input'
}

/** 拆除录音链路（不触发 recognize） */
function teardownRecorder() {
  try {
    processor?.disconnect()
  } catch {
    /* ignore */
  }
  processor = null
  void audioCtx?.close().catch(() => {})
  audioCtx = null
  stream?.getTracks().forEach((t) => t.stop())
  stream = null
  pcmBuffer = []
  vadPending = []
  if (asrStore) asrStore.setMicState('idle')
}

/** 重置会话状态（录音拆除 + phase/activeSource 归位） */
function resetSession() {
  teardownRecorder()
  phase.value = 'idle'
  activeSource.value = null
}

/** 丢弃当前录音：停止但不触发 recognize（spec §3.0 —— 路由/抽屉离开时） */
function discardRecording() {
  const source = activeSource.value
  if (phase.value === 'recognizing') {
    void asrCancel()
  }
  resetSession()
  if (source) void asrStopListening(source)
}

// ── VAD 流（auto 模式）：每 512 samples（30ms @ 16k）喂后端 ──
// 严格串行单飞：一块 invoke 完成才发下一块。Silero 的 h/c 隐状态依赖
// 顺序输入——并发 fire-and-forget 会导致后端锁等待乱序，prob 结果无意义
// （表现：VAD 永不触发 SpeechStarted / TurnCandidate）。
let vadSending = false
function feedVad() {
  if (!asrStore || phase.value !== 'recording' || activeSource.value !== 'auto') return
  if (vadSending || vadPending.length < 512) return
  const block = vadPending.splice(0, 512)
  vadSending = true
  asrVadProcessChunk(block)
    .catch(() => {
      /* VAD 失败不阻塞录音 */
    })
    .finally(() => {
      vadSending = false
      feedVad()
    })
}

/** VAD 检测到一轮说话结束（turn_candidate / turn_sealed）→ 结束 auto 会话 */
async function onVadTurnEnd() {
  console.log('[ASR] VAD turn 事件, activeSource=', activeSource.value, 'phase=', phase.value)
  if (activeSource.value !== 'auto') return
  if (phase.value === 'recording') {
    stop()
  }
}

// ── 能量监测（auto_listen 常开，RMS 超阈值触发 auto 会话） ──
function startEnergyMonitor() {
  if (energyMon) return
  if (!asrStore?.settings.auto_listen || !chatActive.value) return
  navigator.mediaDevices
    .getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } })
    .then((s) => {
      if (!asrStore?.settings.auto_listen || !chatActive.value) {
        s.getTracks().forEach((t) => t.stop())
        return
      }
      const ctx = new AudioContext()
      const src = ctx.createMediaStreamSource(s)
      const analyser = ctx.createAnalyser()
      analyser.fftSize = 1024
      analyser.smoothingTimeConstant = 0.3
      src.connect(analyser)
      const buf = new Uint8Array(analyser.frequencyBinCount)
      const tick = () => {
        if (!asrStore?.settings.auto_listen || !chatActive.value) {
          stopEnergyMonitor()
          return
        }
        if (!energyMon) return
        analyser.getByteFrequencyData(buf)
        // RMS 归一化：byte 0-255 → 0-1，阈值 0.08 约等于明显人声能量
        let sum = 0
        for (let i = 0; i < buf.length; i++) sum += buf[i] * buf[i]
        const rms = Math.sqrt(sum / buf.length) / 128
        if (rms > 0.08 && phase.value === 'idle' && !autoTriggered) {
          autoTriggered = true
          void start('auto').catch(() => {
            autoTriggered = false
          })
          return
        }
        energyMon.raf = requestAnimationFrame(tick)
      }
      energyMon = { ctx, raf: requestAnimationFrame(tick), stream: s }
    })
    .catch(() => {
      /* mic 不可用：能量监测静默降级 */
    })
}

function stopEnergyMonitor() {
  if (!energyMon) return
  cancelAnimationFrame(energyMon.raf)
  void energyMon.ctx.close().catch(() => {})
  energyMon.stream.getTracks().forEach((t) => t.stop())
  energyMon = null
}

// ── 会话生命周期 ────────────────────────────────────────────
async function start(source: AsrSource) {
  if (!chatActive.value) return
  if (activeSource.value !== null) {
    throw new Error('ASR session busy')
  }
  activeSource.value = source
  phase.value = 'recording'
  asrStore?.setMicState('recording')
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        sampleRate: 16000,
        channelCount: 1,
        echoCancellation: true,
        noiseSuppression: true,
      },
    })
    audioCtx = new AudioContext({ sampleRate: 16000 })
    const src = audioCtx.createMediaStreamSource(stream)
    processor = audioCtx.createScriptProcessor(1024, 1, 1)
    src.connect(processor)
    // 输出接零增益节点而非 destination，避免把采集流回放
    const silence = audioCtx.createGain()
    silence.gain.value = 0
    processor.connect(silence)
    silence.connect(audioCtx.destination)
    processor.onaudioprocess = (e) => {
      const data = e.inputBuffer.getChannelData(0)
      pcmBuffer.push(...data)
      if (source === 'auto') {
        vadPending.push(...data)
        // 上限保护：串行速率低于产生速率时丢弃最旧（8192 块 ≈ 4 分钟音频，
        // VAD 端点检测只需要最近的音频）
        if (vadPending.length > 8192) {
          vadPending.splice(0, vadPending.length - 8192)
        }
        feedVad()
      }
    }
    await asrStartListening(source)
  } catch (err: unknown) {
    const name = (err as { name?: string }).name
    console.warn('[ASR] start failed:', err)
    if (name === 'NotAllowedError' || name === 'NotReadableError') {
      asrStore?.setMicState('denied')
    }
    resetSession()
    throw err
  }
}

/** 手动结束（mic 按钮 / 快捷键松开 / VAD turn 结束）：停止 → 识别 → 处理 */
function stop() {
  if (phase.value !== 'recording') return
  const source = activeSource.value
  if (!source) return
  phase.value = 'recognizing'
  // 先拿走 PCM 再拆录音链路（teardownRecorder 会清空 pcmBuffer）
  const captured = pcmBuffer
  teardownRecorder()
  void asrStopListening(source)
  void doRecognize(source, captured)
}

/** 把录音 PCM 合成 WAV 送识别，成功后 handle() */
async function doRecognize(source: AsrSource, captured: number[]) {
  try {
    const wav = pcmToWavPcm16(captured)
    if (wav.byteLength <= 44) {
      // 纯静音（无采样）：直接放弃，不浪费一次识别调用
      resetSession()
      if (source === 'auto') {
        autoTriggered = false
        startEnergyMonitor()
      }
      return
    }
    const result = await asrRecognizeWav({
      providerId: asrStore?.settings.active_provider ?? 'openai-whisper',
      wavBytes: Array.from(wav),
      languageHint: null,
    })
    asrStore?.onResult(result)
    handle(result.text, source)
  } catch (err) {
    console.error('[ASR] recognize failed:', err)
    resetSession()
    if (source === 'auto') {
      autoTriggered = false
      startEnergyMonitor()
    }
  }
}

/**
 * 识别后处理：填入 / 自动发送 / 入队
 * 三模式（asrStore.settings.send_mode）：
 * - fill_only: emit window 'asr-text' event，GameDialog 监听后填 inputMessage
 * - auto_send: 直接 invoke send_chat_message；生成锁忙时降级 queue
 * - queue: 入 pendingAsrQueue，AI 生成结束后 flush
 */
function handle(text: string, source: AsrSource) {
  const mode = asrStore?.settings.send_mode ?? 'fill_only'
  // pendingAsrQueue 兜底：gameStore 不一定有这字段
  const queue = ((gameStore as unknown as { pendingAsrQueue?: string[] }).pendingAsrQueue ??= [])
  if (mode === 'fill_only') {
    window.dispatchEvent(new CustomEvent('asr-text', { detail: text }))
  } else if (mode === 'auto_send') {
    if (isChatBusy()) {
      queue.push(text)
    } else {
      void invoke('send_chat_message', { text, screenshotBase64: null })
    }
  } else if (mode === 'queue') {
    queue.push(text)
  }
  resetSession()
  // auto 模式本轮结束：复位触发标志 + 重新开始能量监听
  if (source === 'auto') {
    autoTriggered = false
    startEnergyMonitor()
  }
}

// ── 惰性初始化（首次调用时执行一次，注册全局监听） ──────────
let initialized = false
function ensureInit() {
  if (initialized) return
  initialized = true
  route = useRoute()
  uiStore = useUIStore()
  asrStore = useAsrStore()
  gameStore = useGameStore()

  // 与后端同步设置：store 可能被 persist 恢复了 localStorage 旧值
  // （如旧 active_provider），不 load 会导致识别走到错误的 provider。
  // load 完成后热键/auto_listen 的 watch 会自动响应新值。
  void asrStore.load().catch((e) => console.warn('[ASR] load settings failed:', e))

  // VAD 事件（经 store 中转，与 tauri-events.ts 的全局监听共用 store 字段）
  watch(
    () => asrStore?.vadEvent ?? null,
    (e: VadEvent | null) => {
      if (!e) return
      if (e.type === 'turn_candidate' || e.type === 'turn_sealed') {
        void onVadTurnEnd()
      }
    },
  )

  // ── 系统级全局快捷键（后台可触发） ──
  // 后端 RegisterHotKey 注册/注销，设置启用或组合变化时同步
  watch(
    () => [asrStore?.settings.hotkey_enabled, asrStore?.settings.hotkey_combination] as const,
    ([enabled, combo]) => {
      if (enabled && combo) {
        void asrRegisterHotkey(combo).catch((e) => {
          console.warn('[ASR] 注册全局快捷键失败:', e)
        })
      } else {
        void asrUnregisterHotkey().catch(() => {
          /* 未注册时注销失败可忽略 */
        })
      }
    },
    { immediate: true },
  )
  // 按下 → 开始录音；释放 → 停止（RegisterHotKey 只有按下通知，释放由后端轮询检测）
  listen('asr://hotkey_down', () => {
    if (chatActive.value && phase.value === 'idle') {
      void start('hotkey').catch(() => {
        /* 会话忙时静默忽略 */
      })
    }
  })
  listen('asr://hotkey_up', () => {
    if (activeSource.value === 'hotkey') {
      stop()
    }
  })

  // 路由/抽屉变化：丢弃当前会话 + 能量监测启停（spec §3.0 门控）
  watch(chatActive, (active) => {
    if (!active) {
      discardRecording()
      stopEnergyMonitor()
    } else if (asrStore?.settings.auto_listen) {
      startEnergyMonitor()
    }
  })
}

export function useAsrInput() {
  ensureInit()
  return {
    phase,
    activeSource,
    chatActive,
    start,
    stop,
    discardRecording,
    handle,
    cancel: () => asrCancel(),
  }
}

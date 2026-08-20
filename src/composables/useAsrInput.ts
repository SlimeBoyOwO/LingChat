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
import { pcmToWavPcm16, trimSilencePcm } from '@/utils/asrAudio'

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
/** 移动端菜单展开状态（GameDialog 在 watch 中同步，§1.5 判定） */
let mobileMenuOpen = false
/** 短暂显示锁：识别后填入 inputMessage 到自动 send 之间的窗口期，期间 ASR 禁用（§1.10） */
let asrLockedUntil = 0
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
  vadSentFrames = 0
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

// ── ASR 可用性门控（§1 全 8 项） ──────────────────────────────
// 综合判定当前能否启动 ASR 录音（所有禁用条件取 OR）：
// 1-3. currentStatus ∈ {thinking, responding, presenting}
// 4.    command === 'touch'（触摸模式）
// 5.    showMobileMenu === true（移动端菜单展开）
// 6.    route.path !== '/chat'
// 7.    uiStore.showSettings === true
// 8.    runningScript && choices.length > 0（剧本选择分支）
// 任何一项满足即视为不可用。start() / startEnergyMonitor RMS 触发 / 按钮 enable 都查它。
function canStartAsr(): boolean {
  if (!route || !uiStore || !gameStore) return false
  // 6 + 7：路由/抽屉门控（chatActive 已是这两项的合成）
  if (route.path !== '/chat' || uiStore.showSettings) return false
  // 9：LoadingTransition 启动动画未完成（§1.9）
  if (!gameStore.loadingComplete) return false
  // 1-3：核心对话状态
  if (gameStore.currentStatus !== 'input') return false
  // 4：触摸模式
  if (gameStore.command === 'touch') return false
  // 5：移动端菜单展开
  if (mobileMenuOpen) return false
  // 8：剧本选择分支
  const script = (gameStore as unknown as { runningScript?: { choices?: unknown[] } })
    .runningScript
  if (script && Array.isArray(script.choices) && script.choices.length > 0) return false
  // 10：识别结果短暂显示锁（fill_only 模式填入 inputMessage 到自动 send 之间的窗口期）
  if (Date.now() < asrLockedUntil) return false
  return true
}

/** 同步录音 + 能量监测状态到最新可用性（任一 watch 触发时调用） */
function updateAsrAvailability(): void {
  const wantMonitor = canStartAsr() && (asrStore?.settings.auto_listen ?? false)
  if (wantMonitor) {
    startEnergyMonitor()
  } else {
    // 不可用 → 拆掉在飞录音 + 停能量监测
    if (phase.value === 'recording' || phase.value === 'recognizing') {
      discardRecording()
    }
    stopEnergyMonitor()
  }
}

/** GameDialog 调用：同步移动端菜单展开状态（§1.5） */
export function setMobileMenuOpen(open: boolean): void {
  mobileMenuOpen = open
  updateAsrAvailability()
}

/**
 * GameDialog 调用：锁定 ASR 一段时间（识别结果填入 inputMessage 后短暂显示用，§1.10）。
 * 显示期间用户不能再次触发录音（避免 nextTick 期间又来一段覆盖识别结果）。
 */
export function lockAsrForDisplay(ms: number): void {
  asrLockedUntil = Date.now() + ms
  updateAsrAvailability()
}

// ── VAD 流（auto 模式）：每 512 samples（30ms @ 16k）喂后端 ──
// 严格串行单飞：一块 invoke 完成才发下一块。Silero 的 h/c 隐状态依赖
// 顺序输入——并发 fire-and-forget 会导致后端锁等待乱序，prob 结果无意义
// （表现：VAD 永不触发 SpeechStarted / TurnCandidate）。
let vadSending = false
/** 诊断：已发送的 VAD 块数（用于降频日志） */
let vadSentFrames = 0
function feedVad() {
  if (!asrStore || phase.value !== 'recording' || activeSource.value !== 'auto') return
  if (vadSending || vadPending.length < 512) return
  const block = vadPending.splice(0, 512)
  vadSending = true
  // 诊断日志：前 10 块 + 每秒 1 条（33 块），确认 VAD 流在走
  if (vadSentFrames < 10 || vadSentFrames % 33 === 0) {
    console.log(`[ASR/VAD] feedVad #${vadSentFrames} 发送 ${block.length} samples`)
  }
  vadSentFrames++
  asrVadProcessChunk(block)
    .catch((e) => {
      // VAD 失败不阻塞录音，但错误不能静默——暴露给调试者
      console.warn('[ASR/VAD] feedVad 失败:', e)
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
  // §1 全 8 项 + auto_listen 设置：任何一项不满足则不开
  if (!asrStore?.settings.auto_listen) return
  if (!canStartAsr()) return
  console.log('[ASR] startEnergyMonitor 启动 (auto_listen=on, canStartAsr=true)')
  navigator.mediaDevices
    .getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } })
    .then((s) => {
      if (!asrStore?.settings.auto_listen || !chatActive.value) {
        console.log('[ASR] startEnergyMonitor 启动后条件失效，关闭 stream')
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
          // 二次校验：AI 可能在本帧之间从 input 进入 thinking，RMS 触发时已不可用
          if (!canStartAsr()) {
            energyMon.raf = requestAnimationFrame(tick)
            return
          }
          console.log(`[ASR] energy trigger: rms=${rms.toFixed(3)} > 0.08, start('auto')`)
          autoTriggered = true
          void start('auto').catch((err) => {
            console.warn('[ASR] start(auto) failed, reset autoTriggered:', err)
            autoTriggered = false
          })
          return
        }
        energyMon.raf = requestAnimationFrame(tick)
      }
      energyMon = { ctx, raf: requestAnimationFrame(tick), stream: s }
      console.log('[ASR] startEnergyMonitor 已建立 analyser, tick loop 开始')
    })
    .catch((err) => {
      console.warn('[ASR] startEnergyMonitor getUserMedia 失败:', err)
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
  // §1 全 8 项门控；任何一项不满足即拒绝启动
  if (!canStartAsr()) return
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
    // 裁剪首尾静音：录音含触发前的环境声 + VAD 停顿尾巴，只送语音段
    const trimmed = trimSilencePcm(captured)
    const wav = pcmToWavPcm16(trimmed)
    if (wav.byteLength <= 44) {
      // 纯静音（无采样）：直接放弃，不浪费一次识别调用
      resetSession()
      if (source === 'auto') {
        autoTriggered = false
        updateAsrAvailability()
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
      updateAsrAvailability()
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
  // §4: 识别请求在飞行中 AI 可能从 input 进入 thinking/responding/presenting
  // 返回时 currentStatus 已变 → 识别结果丢弃（不填入 / 不发送 / 不入队）
  if (!gameStore || gameStore.currentStatus !== 'input') {
    console.log(
      `[ASR] handle drop: status=${gameStore?.currentStatus}, text="${text.slice(0, 30)}"`,
    )
    resetSession()
    if (source === 'auto') {
      autoTriggered = false
      updateAsrAvailability()
    }
    return
  }
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
  // auto 模式本轮结束：复位触发标志 + 通过统一门控重新评估能量监测
  if (source === 'auto') {
    autoTriggered = false
    updateAsrAvailability()
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
    if (canStartAsr() && phase.value === 'idle') {
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

  // 路由/抽屉变化（§1.6/7）：通过统一 gate 同步录音/能量监测
  // immediate:true 让首次进入 /chat（或刚初始化）时立刻同步 energy monitor 状态
  watch(
    chatActive,
    (active) => {
      console.log(`[ASR] chatActive -> ${active}`)
      updateAsrAvailability()
    },
    { immediate: true },
  )
  // auto_listen 设置开关（用户在设置页切换时立即启停）
  watch(
    () => asrStore?.settings.auto_listen,
    (enabled) => {
      console.log(`[ASR] auto_listen -> ${enabled}`)
      updateAsrAvailability()
    },
    { immediate: true },
  )
  // 触摸模式（§1.4）
  watch(
    () => gameStore?.command,
    (cmd) => {
      console.log(`[ASR] command -> ${cmd}`)
      updateAsrAvailability()
    },
    { immediate: true },
  )
  // currentStatus（§1.1-3：thinking/responding/presenting）
  watch(
    () => gameStore?.currentStatus,
    (status) => {
      console.log(`[ASR] currentStatus -> ${status}`)
      updateAsrAvailability()
    },
    { immediate: true },
  )
  // 剧本选择分支（§1.8）
  watch(
    () =>
      (gameStore as unknown as { runningScript?: { choices?: unknown[] } })?.runningScript
        ?.choices?.length ?? 0,
    (n) => {
      console.log(`[ASR] runningScript.choices.length -> ${n}`)
      updateAsrAvailability()
    },
    { immediate: true },
  )
  // LoadingTransition 启动动画完成（§1.9）
  watch(
    () => gameStore?.loadingComplete,
    (done) => {
      console.log(`[ASR] loadingComplete -> ${done}`)
      updateAsrAvailability()
    },
    { immediate: true },
  )
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
    canStartAsr,
  }
}

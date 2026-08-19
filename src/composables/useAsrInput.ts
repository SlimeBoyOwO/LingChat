import { ref, computed, shallowRef, watch, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'

import { useUIStore } from '@/stores/modules/ui/ui'
import { useAsrStore } from '@/stores/modules/settings/asr'
import { useGameStore } from '@/stores/modules/game'
import {
  asrStartListening,
  asrStopListening,
  asrRecognizeWav,
  asrCancel,
  type AsrSource,
  type AsrResult,
} from '@/api/services/asr'

/**
 * 统一 ASR 输入入口：三种触发源共用同一会话生命周期。
 *
 * 三种触发源：
 * - Button: GameDialog.vue / PetMode.vue 的 mic 按钮
 * - Hotkey: useGlobalHotkey.ts 注册的全局快捷键
 * - Auto: asrStore.settings.auto_listen=true 时由 energy monitor 触发
 *
 * 窗口活跃门控：仅当 chatActive=true（/chat 路由 + 设置抽屉未开）时启用。
 * 失败降级：mic 不可用时 fail-open（不抛错到用户），退化为手动按钮 + 不录。
 *
 * 队列设计说明：项目里没有专门的 useChatStore（聊天状态由 useGameStore.currentStatus
 * 体现：'input' = 空闲可输入，'thinking'/'responding'/'presenting' = 生成中）。
 * 因此用 gameStore 顶一个非类型化字段 pendingAsrQueue 做兜底，作为 ASR→chat 的
 * 跨组件排队通道（GameDialog 在 currentStatus 转回 'input' 时 flush）。
 */
export function useAsrInput() {
  const route = useRoute()
  const uiStore = useUIStore()
  const asrStore = useAsrStore()
  const gameStore = useGameStore()

  // 状态
  const phase = ref<'idle' | 'recording' | 'recognizing'>('idle')
  const activeSource = shallowRef<AsrSource | null>(null)
  const recorder = shallowRef<MediaRecorder | null>(null)
  const stream = shallowRef<MediaStream | null>(null)
  const chunks = shallowRef<Blob[]>([])

  // 关键修正：spec §3.0 用 showSettings，不是 settingsOpen
  const chatActive = computed(() => route.path === '/chat' && !uiStore.showSettings)

  // chat 等价"generating"：非 input 即视为生成中
  const chatBusy = computed(() => gameStore.currentStatus !== 'input')

  function cleanup() {
    try {
      recorder.value?.stop()
    } catch {
      /* ignore */
    }
    stream.value?.getTracks().forEach((t) => t.stop())
    recorder.value = null
    stream.value = null
    chunks.value = []
    asrStore.setMicState('idle')
  }

  /** webm/opus blob → 16kHz mono PCM16 WAV bytes */
  async function webmToWavPcm16Mono16k(blob: Blob): Promise<Uint8Array> {
    const arrayBuffer = await blob.arrayBuffer()
    const audioCtx = new (window.AudioContext ||
      (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)()
    const audioBuffer = await audioCtx.decodeAudioData(arrayBuffer)
    await audioCtx.close()

    const targetRate = 16000
    const numSamples = Math.ceil(audioBuffer.duration * targetRate)
    const offlineCtx = new OfflineAudioContext(1, numSamples, targetRate)
    const source = offlineCtx.createBufferSource()
    source.buffer = audioBuffer
    source.connect(offlineCtx.destination)
    source.start()
    const rendered = await offlineCtx.startRendering()
    const channelData = rendered.getChannelData(0)

    const pcm16 = new Int16Array(channelData.length)
    for (let i = 0; i < channelData.length; i++) {
      const s = Math.max(-1, Math.min(1, channelData[i]))
      pcm16[i] = s < 0 ? s * 0x8000 : s * 0x7fff
    }

    // WAV header (44 bytes)
    const header = new ArrayBuffer(44)
    const view = new DataView(header)
    writeAscii(view, 0, 'RIFF')
    view.setUint32(4, 36 + pcm16.byteLength, true)
    writeAscii(view, 8, 'WAVE')
    writeAscii(view, 12, 'fmt ')
    view.setUint32(16, 16, true) // fmt chunk size
    view.setUint16(20, 1, true) // PCM
    view.setUint16(22, 1, true) // mono
    view.setUint32(24, targetRate, true)
    view.setUint32(28, targetRate * 2, true)
    view.setUint16(32, 2, true) // block align
    view.setUint16(34, 16, true) // bits per sample
    writeAscii(view, 36, 'data')
    view.setUint32(40, pcm16.byteLength, true)

    // 拼接 header + pcm16（避免 .map((_,i) => ...) 的大数组构造）
    const out = new Uint8Array(header.byteLength + pcm16.byteLength)
    out.set(new Uint8Array(header), 0)
    out.set(new Uint8Array(pcm16.buffer), header.byteLength)
    return out
  }

  function writeAscii(view: DataView, offset: number, s: string) {
    for (let i = 0; i < s.length; i++) {
      view.setUint8(offset + i, s.charCodeAt(i))
    }
  }

  async function start(source: AsrSource) {
    if (!chatActive.value) return
    if (activeSource.value !== null) {
      throw new Error('ASR session busy')
    }
    activeSource.value = source
    phase.value = 'recording'
    asrStore.setMicState('recording')
    try {
      stream.value = await navigator.mediaDevices.getUserMedia({
        audio: {
          sampleRate: 16000,
          channelCount: 1,
          echoCancellation: true,
          noiseSuppression: true,
        },
      })
      const mr = new MediaRecorder(stream.value, {
        mimeType: 'audio/webm;codecs=opus',
      })
      chunks.value = []
      mr.ondataavailable = (e) => {
        if (e.data.size > 0) chunks.value.push(e.data)
      }
      mr.onstop = () => onStop(source)
      recorder.value = mr
      await asrStartListening(source)
      mr.start(100)
    } catch (err: unknown) {
      const name = (err as { name?: string }).name
      console.warn('[ASR] start failed:', err)
      if (name === 'NotAllowedError' || name === 'NotReadableError') {
        asrStore.setMicState('denied')
      }
      cleanup()
      phase.value = 'idle'
      activeSource.value = null
      throw err
    }
  }

  function stop() {
    if (phase.value !== 'recording') return
    phase.value = 'recognizing'
    try {
      recorder.value?.stop()
    } catch {
      /* ignore */
    }
    stream.value?.getTracks().forEach((t) => t.stop())
    void asrStopListening(activeSource.value as AsrSource)
  }

  async function onStop(source: AsrSource) {
    try {
      const blob = new Blob(chunks.value, { type: 'audio/webm' })
      const wav = await webmToWavPcm16Mono16k(blob)
      const result = await asrRecognizeWav({
        providerId: asrStore.settings.active_provider,
        wavBytes: Array.from(wav),
        languageHint: null,
      })
      asrStore.onResult(result)
      handle(result.text, source)
    } catch (err) {
      console.error('[ASR] recognize failed:', err)
      cleanup()
      phase.value = 'idle'
      activeSource.value = null
    }
  }

  /**
   * 识别后处理：填入 / 自动发送 / 入队
   * 三模式（asrStore.settings.send_mode）：
   * - fill_only: emit window 'asr-text' event，GameDialog 监听后填 inputMessage
   * - auto_send: 直接 invoke send_chat_message；生成锁忙时降级 queue
   * - queue: 入 pendingAsrQueue，AI 生成结束后 flush
   */
  function handle(text: string, _source: AsrSource) {
    const mode = asrStore.settings.send_mode
    // pendingAsrQueue 兜底：gameStore 不一定有这字段
    const queue = ((gameStore as unknown as { pendingAsrQueue?: string[] }).pendingAsrQueue ??= [])
    if (mode === 'fill_only') {
      window.dispatchEvent(new CustomEvent('asr-text', { detail: text }))
    } else if (mode === 'auto_send') {
      if (chatBusy.value) {
        queue.push(text)
      } else {
        void invoke('send_chat_message', { text, screenshotBase64: null })
      }
    } else if (mode === 'queue') {
      queue.push(text)
    }
    cleanup()
    phase.value = 'idle'
    activeSource.value = null
  }

  // 路由/抽屉变化取消当前会话
  watch(chatActive, (active) => {
    if (!active) {
      if (phase.value === 'recording') {
        stop()
      } else if (phase.value === 'recognizing') {
        void asrCancel()
      }
    }
  })

  onUnmounted(() => {
    cleanup()
  })

  return {
    phase,
    activeSource,
    chatActive,
    start,
    stop,
    handle,
    cancel: () => asrCancel(),
  }
}

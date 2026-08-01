/**
 * 全局音频输出设备管理器
 *
 * 基于浏览器标准 API：
 * - 枚举：navigator.mediaDevices.enumerateDevices()（audiooutput）
 * - 切换：HTMLMediaElement.setSinkId() / AudioContext.setSinkId()
 *
 * 通过 MutationObserver 捕获新增 <audio>/<video> 元素、包装 Audio / AudioContext
 * 构造器，实现全局（背景音乐、角色语音、音效、环境音、打字音效等）音频输出设备切换。
 *
 * 设备选择持久化在设置 store 的 audio.outputDeviceId（'' = 跟随系统默认）。
 */
import { ref } from 'vue'
import { useSettingsStore } from '@/stores/modules/settings'

export interface OutputDevice {
  deviceId: string
  label: string
  isDefault: boolean
}

interface SinkTarget {
  setSinkId?: (sinkId: string) => Promise<void>
}

/** 当前环境是否支持切换（enumerateDevices + setSinkId 可用） */
export const supported = ref(false)
/** 输出设备列表 */
export const devices = ref<OutputDevice[]>([])
/** 当前选中设备，'' = 跟随系统默认 */
export const currentDeviceId = ref('')
/** 是否已获得真实设备名（需一次性授权） */
export const labelsAvailable = ref(false)
/** 授权被拒绝（或没有可用的音频输入设备） */
export const permissionDenied = ref(false)
/** 设备列表加载中 */
export const loading = ref(false)

let initialized = false
let observer: MutationObserver | null = null
let originalAudio: typeof Audio | null = null
let originalAudioContext: typeof AudioContext | null = null
let permissionRequested = false
let pendingSink = false
let activationInstalled = false
let defaultDeviceLabel = ''

/** 已注册的活动媒体元素 / AudioContext（切换设备时重新应用） */
const elements = new Set<HTMLMediaElement>()
const contexts = new Set<AudioContext>()

function canSetSink(target: unknown): target is SinkTarget {
  return !!target && typeof (target as SinkTarget).setSinkId === 'function'
}

async function applyToElement(el: HTMLMediaElement) {
  if (!canSetSink(el)) return
  const id = currentDeviceId.value
  const wasPlaying = !el.paused && !!el.currentSrc
  const resumeTime = el.currentTime
  try {
    await el.setSinkId!(id)
    console.debug('[audio-device] setSinkId 成功:', id, 'src:', (el as HTMLAudioElement).src)
    // 跨音频端点类型（如蓝牙↔有线）切换时，Chromium 不会自动重建正在播放的音频流，
    // 需要重启元素（重新加载资源）才会让新设备生效；这里仅对正在播放的元素做重启
    if (wasPlaying && id) {
      restartElement(el, resumeTime)
    }
  } catch (e: any) {
    const name = e && e.name
    if (name === 'NotAllowedError' || name === 'SecurityError') {
      // 缺少用户激活 → 排队，待首次交互时重试
      pendingSink = true
      installActivationFlush()
    } else {
      // 设备已拔出或 ID 不被接受 → 回退系统默认
      console.warn('[audio-device] setSinkId 失败，回退默认:', id, name, e)
      try {
        await el.setSinkId!('')
      } catch {
        /* 忽略 */
      }
    }
  }
}

/** 重启正在播放的音频元素，强制在新设备上重建音频流 */
function restartElement(el: HTMLMediaElement, resumeTime: number) {
  try {
    el.pause()
    el.load()
    if (resumeTime > 0) el.currentTime = resumeTime
    el.play().catch((e) => console.warn('[audio-device] 重启音频元素失败:', e))
  } catch (e) {
    console.warn('[audio-device] 重启音频元素异常:', e)
  }
}

async function applyToContext(ctx: AudioContext) {
  if (!canSetSink(ctx)) return
  try {
    await ctx.setSinkId!(currentDeviceId.value)
  } catch (e: any) {
    const name = e && e.name
    if (name === 'NotAllowedError' || name === 'SecurityError') {
      pendingSink = true
      installActivationFlush()
    }
  }
}

function registerElement(el: HTMLMediaElement) {
  if (elements.has(el)) return
  elements.add(el)
  applyToElement(el)
}

function registerContext(ctx: AudioContext) {
  if (contexts.has(ctx)) return
  contexts.add(ctx)
  applyToContext(ctx)
  try {
    ctx.addEventListener('statechange', () => {
      if (ctx.state === 'closed') contexts.delete(ctx)
    })
  } catch {
    /* 忽略 */
  }
}

function installActivationFlush() {
  if (activationInstalled) return
  activationInstalled = true
  const flush = () => {
    activationInstalled = false
    window.removeEventListener('pointerdown', flush)
    window.removeEventListener('keydown', flush)
    flushPendingSinks()
  }
  window.addEventListener('pointerdown', flush)
  window.addEventListener('keydown', flush)
}

async function flushPendingSinks() {
  if (!pendingSink) return
  pendingSink = false
  const targets: Promise<void>[] = []
  elements.forEach((el) => targets.push(applyToElement(el)))
  contexts.forEach((ctx) => targets.push(applyToContext(ctx)))
  await Promise.allSettled(targets)
}

function installObserver() {
  if (observer || typeof MutationObserver === 'undefined') return
  observer = new MutationObserver((mutations) => {
    for (const m of mutations) {
      for (const node of m.addedNodes) {
        if (node instanceof HTMLMediaElement) {
          registerElement(node)
        } else if (node.nodeType === Node.ELEMENT_NODE) {
          ;(node as Element)
            .querySelectorAll?.('audio, video')
            .forEach((a) => registerElement(a as HTMLMediaElement))
        }
      }
      for (const node of m.removedNodes) {
        if (node instanceof HTMLMediaElement) {
          elements.delete(node)
        } else if (node.nodeType === Node.ELEMENT_NODE) {
          ;(node as Element)
            .querySelectorAll?.('audio, video')
            .forEach((a) => elements.delete(a as HTMLMediaElement))
        }
      }
    }
  })
  observer.observe(document.documentElement, { childList: true, subtree: true })
}

function installAudioWrapper() {
  if (originalAudio || typeof Audio === 'undefined') return
  originalAudio = window.Audio
  const PatchedAudio = function (this: unknown, src?: string) {
    const el = new (originalAudio as typeof Audio)(src)
    registerElement(el)
    return el
  } as unknown as typeof Audio
  PatchedAudio.prototype = originalAudio.prototype
  ;(window as any).Audio = PatchedAudio
}

function installAudioContextWrapper() {
  const AnyCtx = (window as any).AudioContext || (window as any).webkitAudioContext
  if (!AnyCtx || originalAudioContext) return
  const Original = AnyCtx as typeof AudioContext
  originalAudioContext = Original
  const PatchedCtx = function (this: unknown, ...args: unknown[]) {
    const ctx = new Original(...(args as ConstructorParameters<typeof AudioContext>))
    registerContext(ctx)
    return ctx
  } as unknown as typeof AudioContext
  PatchedCtx.prototype = Original.prototype
  ;(window as any).AudioContext = PatchedCtx
  if ((window as any).webkitAudioContext) {
    ;(window as any).webkitAudioContext = PatchedCtx
  }
}

/** 补丁 play()：任何 <audio>/<video> 播放前先应用当前输出设备，确保所有播放路径生效 */
function installPlayPatch() {
  if (typeof HTMLMediaElement === 'undefined' || !('play' in HTMLMediaElement.prototype)) return
  const originalPlay = HTMLMediaElement.prototype.play
  ;(HTMLMediaElement.prototype as any).play = function (this: HTMLMediaElement) {
    const el = this
    if (canSetSink(el) && currentDeviceId.value) {
      // 先应用设备（幂等、失败忽略），再继续播放
      const sinkPromise = Promise.resolve(el.setSinkId!(currentDeviceId.value)).catch(
        () => undefined,
      )
      return sinkPromise.then(() => originalPlay.call(el))
    }
    return originalPlay.call(el)
  }
}

/** 尝试一次性授权，解锁真实设备名（需在用户手势中调用） */
export async function requestDeviceLabels(force = false) {
  if (permissionRequested && !force) return
  permissionRequested = true
  try {
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
      labelsAvailable.value = false
      return
    }
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    stream.getTracks().forEach((t) => t.stop())
    labelsAvailable.value = true
    permissionDenied.value = false
  } catch (e) {
    permissionDenied.value = true
    labelsAvailable.value = false
    console.warn('获取音频设备名称授权失败（可能被拒绝或没有可用的音频输入设备）:', e)
  }
}

/** 重新扫描输出设备列表；forcePermission 为 true 时先尝试解锁设备名 */
export async function refreshDevices(forcePermission = false) {
  if (!supported.value) return
  loading.value = true
  try {
    if (forcePermission) {
      await requestDeviceLabels(true)
    }
    const all = await navigator.mediaDevices.enumerateDevices()
    const outputs = all.filter((d) => d.kind === 'audiooutput')
    const defaultEntry = outputs.find((d) => d.deviceId === 'default')
    defaultDeviceLabel = defaultEntry?.label || ''
    const hasLabels = outputs.some((d) => !!d.label)
    labelsAvailable.value = hasLabels
    const realOutputs = outputs.filter(
      (d) => d.deviceId !== 'default' && d.deviceId !== 'communications',
    )
    devices.value = realOutputs.map((d, i) => ({
      deviceId: d.deviceId,
      label: d.label || `输出设备 ${i + 1}`,
      isDefault: !!defaultDeviceLabel && d.label === defaultDeviceLabel,
    }))
    // 持久化的设备已不存在（且确有真实设备列表，避免授权未就绪时误重置）→ 回退系统默认
    if (
      currentDeviceId.value &&
      realOutputs.length > 0 &&
      !devices.value.some((d) => d.deviceId === currentDeviceId.value)
    ) {
      await setDevice('')
    }
  } catch (e) {
    console.error('枚举音频输出设备失败:', e)
  } finally {
    loading.value = false
  }
}

/** 切换输出设备（'' = 跟随系统默认），并持久化到设置 */
export async function setDevice(id: string) {
  currentDeviceId.value = id
  try {
    const settingsStore = useSettingsStore()
    settingsStore.update('audio.outputDeviceId', id)
  } catch (e) {
    console.warn('保存音频输出设备设置失败:', e)
  }
  // 全量重新注册当前 DOM 中的音频元素，确保不遗漏（幂等）
  document
    .querySelectorAll('audio, video')
    .forEach((el) => registerElement(el as HTMLMediaElement))
  pendingSink = false
  const targets: Promise<void>[] = []
  elements.forEach((el) => targets.push(applyToElement(el)))
  contexts.forEach((ctx) => targets.push(applyToContext(ctx)))
  await Promise.allSettled(targets)
  console.debug(
    '[audio-device] setDevice:',
    id,
    '已应用到',
    elements.size,
    '个媒体元素,',
    contexts.size,
    '个 AudioContext',
  )
}

/** 全局初始化（应用启动时调用一次，需在 pinia 就绪后） */
export async function initAudioOutputManager() {
  if (initialized) return
  initialized = true

  const hasApi =
    typeof navigator !== 'undefined' &&
    !!navigator.mediaDevices &&
    typeof navigator.mediaDevices.enumerateDevices === 'function'
  const hasSink =
    typeof HTMLMediaElement !== 'undefined' && 'setSinkId' in HTMLMediaElement.prototype
  supported.value = hasApi && hasSink
  if (!supported.value) return

  try {
    const settingsStore = useSettingsStore()
    currentDeviceId.value = settingsStore.audio.outputDeviceId || ''
  } catch (e) {
    currentDeviceId.value = ''
  }

  // 注册现有媒体元素
  document.querySelectorAll('audio, video').forEach((el) => registerElement(el as HTMLMediaElement))

  installObserver()
  installAudioWrapper()
  installAudioContextWrapper()
  installPlayPatch()

  // 首次扫描（不主动请求授权）
  refreshDevices(false)

  // 设备热插拔时自动刷新
  try {
    navigator.mediaDevices.addEventListener?.('devicechange', () => refreshDevices(false))
  } catch {
    /* 忽略 */
  }

  // 已持久化设备在启动时无用户激活，排队等首次交互再应用
  if (currentDeviceId.value) {
    pendingSink = true
    installActivationFlush()
  }
}

/** 调试辅助：输出管理器当前状态（控制台调用） */
export function debugAudioOutput() {
  return {
    supported: supported.value,
    currentDeviceId: currentDeviceId.value,
    labelsAvailable: labelsAvailable.value,
    permissionDenied: permissionDenied.value,
    devices: devices.value,
    registeredElements: elements.size,
    registeredContexts: contexts.size,
    elementList: [...elements].map((el) => ({
      tag: el.tagName,
      src: (el as HTMLAudioElement).src || '(无src)',
      paused: el.paused,
    })),
  }
}

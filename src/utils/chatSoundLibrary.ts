import { convertFileSrc } from '@tauri-apps/api/core'
import { chatSoundGetAll } from '@/api/services/chatSound'
import type { ChatSoundItem } from '@/api/services/chatSound'

/**
 * 自定义聊天音效库（全局单例）。
 *
 * 聊天打字音效的音源来自 data/game_data/chat_sounds/ 目录：
 * - 目录非空时随机播放其中的音效（TypeWriter 已支持多音源随机）
 * - 目录为空（或非 Tauri 环境加载失败）时回退内置音效
 *
 * 本模块是音效库的唯一数据源：列表拉取/缓存、URL 换算、音频解码缓存
 * 都集中在这里，设置页与 TypeWriter 实例只消费，避免重复 IPC 查询；
 * 主聊天与桌宠各自的 TypeWriter 共享同一份解码结果，常驻内存不随
 * 实例数增长。
 */
export const BUILTIN_CHAT_SOUND_URLS = ['../audio_effects/对话.wav']

let customItems: ChatSoundItem[] = []
let version = 0
let loadPromise: Promise<void> | null = null

// ─── 音频数据缓存（跨 TypeWriter 实例共享）─────────────────
// 原始 ArrayBuffer：fetch 一次全局复用（decodeAudioData 会 detach 传入的
// buffer，因此每次解码都传 slice 副本；解码成功后即可释放原始数据）
const arrayBufferCache = new Map<string, ArrayBuffer>()

// 解码结果按「URL + 采样率」缓存：AudioBuffer 可跨 AudioContext 复用，
// 但采样率不一致时播放会变速变调，因此不同采样率的上下文各自解码
const decodedCache = new Map<string, Map<number, AudioBuffer>>()

/** 音效库当前有效的音源 URL 集合（含内置），用于清理失效缓存 */
function validSoundUrls(): Set<string> {
  const urls = new Set<string>(BUILTIN_CHAT_SOUND_URLS)
  if (customItems.length > 0) {
    for (const url of getChatSoundUrls()) urls.add(url)
  }
  return urls
}

/** 库内容变化（上传/删除）后，清掉不再存在的音源的缓存 */
function pruneCaches(): void {
  const valid = validSoundUrls()
  for (const url of [...arrayBufferCache.keys()]) {
    if (!valid.has(url)) arrayBufferCache.delete(url)
  }
  for (const url of [...decodedCache.keys()]) {
    if (!valid.has(url)) decodedCache.delete(url)
  }
}

/** 从后端拉取自定义聊天音效列表并刷新缓存（唯一的列表查询入口） */
export async function refreshChatSounds(): Promise<void> {
  const load = (async () => {
    try {
      customItems = await chatSoundGetAll()
    } catch {
      // 非 Tauri 环境（浏览器开发模式）或后端暂不可用：保持回退内置音效
      customItems = []
    }
    version++
    pruneCaches()
  })()
  loadPromise = load
  await load
}

/** 确保音效库至少加载过一次（TypeWriter 首次加载音效时调用） */
export async function ensureChatSoundsLoaded(): Promise<void> {
  if (!loadPromise) {
    await refreshChatSounds()
  }
}

/** 自定义聊天音效列表（设置页直接消费） */
export function getChatSoundItems(): ChatSoundItem[] {
  return [...customItems]
}

/** 当前生效的聊天音效 URL 列表（自定义为空时回退内置） */
export function getChatSoundUrls(): string[] {
  if (customItems.length === 0) return [...BUILTIN_CHAT_SOUND_URLS]
  return customItems.map((item) => convertFileSrc(item.url))
}

/** 音效库版本号（每次刷新递增，供 TypeWriter 检测是否需要重新加载） */
export function getChatSoundVersion(): number {
  return version
}

/**
 * 解码单个音效（带全局共享缓存）。
 *
 * - 命中缓存直接返回；未命中则 fetch（ArrayBuffer 只取一次）后解码
 * - 解码结果按采样率分桶缓存，主聊天/桌宠的 AudioContext 采样率相同时
 *   复用同一份 AudioBuffer；不同则各自解码，避免跨采样率变速变调
 * - 失败返回 null，由调用方决定回退策略
 */
export async function decodeChatSound(
  audioContext: AudioContext,
  url: string,
): Promise<AudioBuffer | null> {
  const rate = audioContext.sampleRate
  let byRate = decodedCache.get(url)
  const cached = byRate?.get(rate)
  if (cached) return cached

  try {
    let raw = arrayBufferCache.get(url)
    if (!raw) {
      const response = await fetch(url)
      raw = await response.arrayBuffer()
      arrayBufferCache.set(url, raw)
    }
    const buffer = await audioContext.decodeAudioData(raw.slice(0))
    byRate ??= new Map()
    byRate.set(rate, buffer)
    decodedCache.set(url, byRate)
    // 解码成功后释放原始数据：同采样率上下文走解码缓存即可，
    // 不同采样率的上下文将来重新 fetch（罕见路径，不值得常驻双份内存）
    arrayBufferCache.delete(url)
    return buffer
  } catch {
    return null
  }
}

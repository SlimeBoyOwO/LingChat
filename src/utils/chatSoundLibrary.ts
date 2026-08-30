import { convertFileSrc } from '@tauri-apps/api/core'
import { chatSoundGetAll } from '@/api/services/chatSound'

/**
 * 自定义聊天音效库（全局单例）。
 *
 * 聊天打字音效的音源来自 data/game_data/chat_sounds/ 目录：
 * - 目录非空时随机播放其中的音效（TypeWriter 已支持多音源随机）
 * - 目录为空（或非 Tauri 环境加载失败）时回退内置音效
 */
export const BUILTIN_CHAT_SOUND_URLS = ['../audio_effects/对话.wav']

let customUrls: string[] = []
let version = 0
let loadPromise: Promise<void> | null = null

/** 从后端拉取自定义聊天音效列表并刷新缓存（上传/删除后调用） */
export async function refreshChatSounds(): Promise<void> {
  const load = (async () => {
    try {
      const list = await chatSoundGetAll()
      customUrls = list.map((item) => convertFileSrc(item.url))
    } catch {
      // 非 Tauri 环境（浏览器开发模式）或后端暂不可用：保持回退内置音效
      customUrls = []
    }
    version++
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

/** 当前生效的聊天音效 URL 列表（自定义为空时回退内置） */
export function getChatSoundUrls(): string[] {
  return customUrls.length > 0 ? [...customUrls] : [...BUILTIN_CHAT_SOUND_URLS]
}

/** 音效库版本号（每次刷新递增，供 TypeWriter 检测是否需要重新加载） */
export function getChatSoundVersion(): number {
  return version
}

// stores/ui.ts
import { defineStore } from 'pinia'

// 通知类型
export type NotificationType = 'error' | 'success' | 'info' | 'warning'
export type ScheduleViewType =
  | 'schedule_groups'
  | 'schedule_details'
  | 'todo_groups'
  | 'todo_detail'
  | 'calendar'

// 通知状态接口
interface NotificationState {
  isVisible: boolean
  type: NotificationType
  title: string
  message: string
  avatarUrl: string
  duration: number
}

interface UIState {
  showCharacterTitle: string
  showCharacterSubtitle: string
  showCharacterEmotion: string
  showCharacterLine: string
  showPlayerHintLine: string
  showCharacterThinkLine: string
  showSettings: boolean
  currentSettingsTab: string
  typeWriterSpeed: number
  enableChatEffectSound: boolean
  currentBackground: string
  currentBackgroundEffect: string
  currentBackgroundMusic: string
  currentSoundEffect: string
  currentAvatarAudio: string
  characterVolume: number
  backgroundVolume: number
  bubbleVolume: number
  autoMode: boolean
  showCommandWheel: boolean

  // Schedule 相关状态
  scheduleView: string

  // Notification 相关状态
  notification: NotificationState
  tipsMap: Record<string, { title: string; message: string }>
  tipsAvailable: boolean
  currentCharacterFolder: string
}

// localStorage key
const STORAGE_KEY_CHARACTER_FOLDER = 'lingchat_character_folder'
const DEFAULT_CHARACTER_FOLDER = '诺一钦灵'
const DEFAULT_AVATAR = '/characters/诺一钦灵/头像.png'

// 防抖相关
const notificationDebounceMap = new Map<string, number>()
const DEBOUNCE_MS_NETWORK = 10000 // "未注明的错误" 10秒
const DEBOUNCE_MS_DEFAULT = 3000 // 其他 3秒

let hideTimer: number | null = null

export const useUIStore = defineStore('ui', {
  state: (): UIState => ({
    showCharacterTitle: 'Lovely You',
    showCharacterSubtitle: 'Bilibili',
    showCharacterEmotion: '',
    showCharacterLine: '',
    showPlayerHintLine: '',
    showCharacterThinkLine: 'Ling Ling Thinking...',
    showSettings: false,
    currentSettingsTab: 'text',
    typeWriterSpeed: 50,
    enableChatEffectSound: true,
    currentBackground: '@/assets/images/default_bg.jpg',
    currentBackgroundEffect: 'StarField',
    currentBackgroundMusic: 'None',
    currentSoundEffect: 'None',
    currentAvatarAudio: 'None',
    characterVolume: 80,
    backgroundVolume: 80,
    bubbleVolume: 80,
    autoMode: false,
    showCommandWheel: true,

    // Schedule 相关状态
    scheduleView: 'schedule_groups',

    // Notification 初始状态
    notification: {
      isVisible: false,
      type: 'info',
      title: '',
      message: '',
      avatarUrl: DEFAULT_AVATAR,
      duration: 3000,
    },
    tipsMap: {},
    tipsAvailable: false,
    currentCharacterFolder:
      localStorage.getItem(STORAGE_KEY_CHARACTER_FOLDER) || DEFAULT_CHARACTER_FOLDER,
  }),

  actions: {
    toggleSettings(show: boolean) {
      this.showSettings = show
    },
    setSettingsTab(tab: string) {
      this.currentSettingsTab = tab
    },
    toggleCommandWheel(show: boolean) {
      this.showCommandWheel = show
    },

    // ========== Notification Actions ==========

    /**
     * 加载角色专属提示
     */
    async loadCharacterTips(folderName: string): Promise<boolean> {
      // 清空之前的提示
      this.tipsMap = {}
      this.tipsAvailable = false
      this.currentCharacterFolder = folderName

      // 保存到 localStorage
      localStorage.setItem(STORAGE_KEY_CHARACTER_FOLDER, folderName)

      // 尝试加载指定角色的 tips
      await this._loadTipsFromFolder(folderName)

      return this.tipsAvailable
    },

    /**
     * 从指定文件夹加载 tips（内部方法）
     */
    async _loadTipsFromFolder(folderName: string): Promise<boolean> {
      try {
        const response = await fetch(`/characters/${folderName}/tips.txt`)

        if (!response.ok) {
          console.log(`⚠️ 角色 ${folderName} 没有 tips.txt`)
          return false
        }

        const text = await response.text()
        const newTipsMap: Record<string, { title: string; message: string }> = {}

        // 解析 txt 格式：代码 = 标题 | 内容
        text.split('\n').forEach((line) => {
          line = line.trim()
          if (!line || line.startsWith('#')) return

          const [code, content] = line.split('=').map((s) => s.trim())
          if (code && content) {
            const [title, message] = content.split('|').map((s) => s.trim())
            if (title && message) {
              newTipsMap[code] = { title, message }
            }
          }
        })

        // 只有有内容才算加载成功
        if (Object.keys(newTipsMap).length === 0) {
          console.log(`⚠️ 角色 ${folderName} 的 tips.txt 为空`)
          return false
        }

        this.tipsMap = newTipsMap
        this.tipsAvailable = true
        console.log(`✅ 已加载角色 ${folderName} 的提示:`, this.tipsMap)
        return true
      } catch (error) {
        console.log(`⚠️ 加载角色 ${folderName} 的提示失败:`, error)
        return false
      }
    },

    /**
     * 显示通知（通用方法）
     */
    showNotification(options: {
      type?: NotificationType
      title?: string
      message?: string
      avatarUrl?: string
      duration?: number
      skipTipsCheck?: boolean // 跳过 tips 检查（用于网络错误等必须显示的通知）
    }) {
      const {
        type = 'info',
        title = '',
        message = '',
        avatarUrl,
        duration = 3000,
        skipTipsCheck = false,
      } = options

      // 如果当前角色没有配置 tips.txt，且没有跳过检查，则不显示弹窗
      if (!this.tipsAvailable && !skipTipsCheck) {
        console.log('跳过弹窗：当前角色没有配置 tips.txt')
        return
      }

      const now = Date.now()
      const notificationKey = `${title}:${message}`

      // 判断是否为"未注明的错误"，使用更长的防抖时间
      const isDefaultError = title === '未注明的错误'
      const debounceMs = isDefaultError ? DEBOUNCE_MS_NETWORK : DEBOUNCE_MS_DEFAULT

      // 防抖检查
      const lastTime = notificationDebounceMap.get(notificationKey) || 0
      if (now - lastTime < debounceMs) {
        console.log(`跳过重复通知：${title}（${debounceMs / 1000}秒内已显示过）`)
        return
      }

      notificationDebounceMap.set(notificationKey, now)

      // 清除之前的定时器
      if (hideTimer) {
        clearTimeout(hideTimer)
      }

      // 更新通知状态
      this.notification = {
        isVisible: true,
        type,
        title,
        message,
        avatarUrl: avatarUrl || `/characters/${this.currentCharacterFolder}/头像.png`,
        duration,
      }

      // 自动隐藏
      if (duration > 0) {
        hideTimer = window.setTimeout(() => {
          this.hideNotification()
        }, duration)
      }
    },

    /**
     * 隐藏通知
     */
    hideNotification() {
      this.notification.isVisible = false
      if (hideTimer) {
        clearTimeout(hideTimer)
        hideTimer = null
      }
    },

    /**
     * 显示错误通知（支持错误代码自动翻译）
     */
    showError(options: {
      errorCode?: string
      statusCode?: number
      title?: string
      message?: string
      avatarUrl?: string
      duration?: number
    }) {
      const { errorCode, statusCode, title, message, avatarUrl, duration = 3000 } = options

      let finalTitle = title || '错误'
      let finalMessage = message || '发生了未知错误'

      // 优先使用错误代码查询
      if (errorCode) {
        const tip = this.tipsMap[errorCode] ||
          this.tipsMap['default_error'] || { title: '错误', message: '发生了未知错误' }
        finalTitle = title || tip.title
        finalMessage = message || tip.message
      }
      // 其次使用 HTTP 状态码
      else if (statusCode) {
        const code = statusCode.toString()
        const httpCode = code + '_http'
        const tip = this.tipsMap[httpCode] || this.tipsMap[code]
        if (tip) {
          finalTitle = title || tip.title
          finalMessage = message || tip.message
        }
      }

      // 网络错误必须显示，不受 tips 配置限制
      const isNetworkError = errorCode === 'network_error'

      this.showNotification({
        type: 'error',
        title: finalTitle,
        message: finalMessage,
        avatarUrl,
        duration,
        skipTipsCheck: isNetworkError,
      })
    },

    /**
     * 显示成功通知
     */
    showSuccess(options: {
      title?: string
      message?: string
      avatarUrl?: string
      duration?: number
    }) {
      this.showNotification({ ...options, type: 'success' })
    },

    /**
     * 显示信息通知
     */
    showInfo(options: { title?: string; message?: string; avatarUrl?: string; duration?: number }) {
      this.showNotification({ ...options, type: 'info' })
    },

    /**
     * 显示警告通知
     */
    showWarning(options: {
      title?: string
      message?: string
      avatarUrl?: string
      duration?: number
    }) {
      this.showNotification({ ...options, type: 'warning' })
    },

    /**
     * 获取角色切换提示
     */
    getSwitchTip(type: 'success' | 'fail') {
      const key = type === 'success' ? 'switch_success' : 'switch_fail'
      return (
        this.tipsMap[key] || {
          title: type === 'success' ? '切换成功' : '切换失败',
          message: type === 'success' ? '角色已切换' : '切换时出了问题',
        }
      )
    },

    /**
     * 获取角色刷新提示
     */
    getRefreshTip(type: 'success' | 'fail') {
      const key = type === 'success' ? 'refresh_success' : 'refresh_fail'
      return (
        this.tipsMap[key] || {
          title: type === 'success' ? '刷新成功' : '刷新失败',
          message: type === 'success' ? '角色列表已成功刷新！' : '刷新时出了问题',
        }
      )
    },
  },
})

// 标记是否已初始化
let initialized = false

// 初始化函数：在首次使用时调用
export function initUIStore() {
  if (initialized) return
  initialized = true

  const store = useUIStore()
  store.loadCharacterTips(store.currentCharacterFolder)
}

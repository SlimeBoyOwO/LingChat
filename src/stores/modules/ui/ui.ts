// stores/ui.ts
import { defineStore } from 'pinia'
import { useSettingsStore } from '../settings'

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
  showCharacterMotionText: string
  showPlayerHintLine: string
  showCharacterThinkLine: string
  showSettings: boolean
  currentSettingsTab: string

  currentBackgroundTransition: number
  currentPresentPic: string
  currentPresentPicScale: number
  currentBackgroundMusic: string
  bgMusicMode: 'loop-list' | 'loop-single' | 'random'
  bgMusicPaused: boolean
  bgMusicStoped: boolean

  currentSoundEffect: string
  currentAvatarAudio: string
  autoMode: boolean

  // 环境音轨道列表（多轨并行，最多8轨）
  ambientTracks: Array<{
    id: string         // 唯一标识（基于时间戳+随机数）
    src: string        // 音频文件URL
    name?: string      // 显示名称（可选，回退到从路径推断）
    volume: number     // 单轨音量 0-100
    loop: boolean      // 是否循环
    paused?: boolean   // 是否暂停
    fade?: boolean     // 是否启用淡入淡出
  }>

  // 视口响应式追踪（全局唯一 resize 监听，组件直接读值）
  viewportWidth: number
  viewportHeight: number

  // Schedule 相关状态
  scheduleView: string

  // Notification 相关状态
  notification: NotificationState
  tipsMap: Record<string, { title: string; message: string }>
  tipsAvailable: boolean

  // 背景音乐结束时间戳，用于触发音乐切换
  _musicEndTime: number
}

// 默认 avatar
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
    showCharacterMotionText: '',
    showPlayerHintLine: '',
    showCharacterThinkLine: 'Ling Ling Thinking...',
    showSettings: false,
    currentSettingsTab: 'text',
    currentBackgroundTransition: 300,
    currentPresentPic: '',
    currentPresentPicScale: 1,

    currentBackgroundMusic: 'None',
    bgMusicMode: 'loop-single',
    bgMusicPaused: false,
    bgMusicStoped: false,

    currentSoundEffect: 'None',
    currentAvatarAudio: 'None',
    autoMode: false,

    // 环境音轨道列表初始值
    ambientTracks: [],

    // 视口响应式追踪
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight,

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

    // 背景音乐结束时间戳
    _musicEndTime: 0,
  }),

  getters: {
    currentBackground(): string {
      return useSettingsStore().currentBackground
    },
    // 从 settings store 获取设置值（向后兼容）
    typeWriterSpeed(): number {
      return useSettingsStore().textSpeed
    },
    enableChatEffectSound(): boolean {
      return useSettingsStore().chatEffectSound
    },
    currentBackgroundEffect(): string {
      return useSettingsStore().backgroundEffect
    },
    characterVolume(): number {
      return useSettingsStore().characterVolume
    },
    backgroundVolume(): number {
      return useSettingsStore().backgroundVolume
    },
    bubbleVolume(): number {
      return useSettingsStore().bubbleVolume
    },
    achievementVolume(): number {
      return useSettingsStore().achievementVolume
    },
    // 从 settings store 获取全局环境音音量
    ambientVolume(): number {
      return useSettingsStore().ambientVolume
    },
    // 角色文件夹（从 settings store 获取）
    currentCharacterFolder(): string {
      return useSettingsStore().characterFolder
    },
    // 视口宽高比
    aspectRatio(): number {
      return this.viewportWidth / this.viewportHeight
    },
    // 窄屏判断（竖屏 / 移动端）
    isNarrowScreen(): boolean {
      return this.aspectRatio < 1.0
    },
  },

  actions: {
    setCurrentBackground(background: string) {
      useSettingsStore().setCurrentBackground(background)
    },
    // 设置背景效果（写入 settings store）
    setBackgroundEffect(effect: string) {
      useSettingsStore().setBackgroundEffect(effect)
    },
    // 设置对话音效开关（写入 settings store）
    setEnableChatEffectSound(enabled: boolean) {
      useSettingsStore().setChatEffectSound(enabled)
    },

    toggleSettings(show: boolean) {
      this.showSettings = show
    },
    setSettingsTab(tab: string) {
      this.currentSettingsTab = tab
    },

    // ========== Notification Actions ==========

    /**
     * 加载角色专属提示
     */
    async loadCharacterTips(folderName: string): Promise<boolean> {
      // 清空之前的提示
      this.tipsMap = {}
      this.tipsAvailable = false

      // 保存到 settings store（自动持久化）
      useSettingsStore().setCharacterFolder(folderName)

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

    /**
     * 处理背景音乐结束事件
     * 当背景音乐播放结束时调用此方法，通知相关组件处理音乐切换
     */
    handleBackgroundMusicEnd() {
      // 触发一个内部状态变化，让SettingsSound组件能够监听到
      // 使用时间戳确保每次都能触发watch
      this._musicEndTime = Date.now()
    },

    // ========== 环境音轨道管理 ==========

    /**
     * 添加环境音轨道
     * 如果已存在相同 src 的轨道则替换，超出上限时移除最早的
     */
    addAmbientTrack(track: { src: string; volume: number; loop: boolean; name?: string; paused?: boolean; fade?: boolean }) {
      const MAX_AMBIENT_TRACKS = 8
      // 提取文件名用于去重（剧本 Assets 和手动导入可能路径不同但文件相同）
      const getFileName = (src: string) => {
        const parts = src.replace(/\\/g, '/').split('/')
        return parts.pop() || src
      }
      const newFileName = getFileName(track.src)
      // 按完整路径或文件名去重，剧本指令优先覆盖手动导入
      this.ambientTracks = this.ambientTracks.filter(t =>
        t.src !== track.src && getFileName(t.src) !== newFileName
      )
      // 超出上限时移除最早的
      if (this.ambientTracks.length >= MAX_AMBIENT_TRACKS) {
        this.ambientTracks.shift()
      }
      const id = `ambient_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`
      this.ambientTracks.push({ id, ...track, paused: track.paused ?? false, fade: track.fade ?? true })
    },

    /**
     * 更新指定环境音轨道的音量
     */
    updateAmbientTrackVolume(id: string, volume: number) {
      const track = this.ambientTracks.find(t => t.id === id)
      if (track) track.volume = volume
    },

    /**
     * 切换环境音轨道暂停状态
     */
    toggleAmbientTrackPause(id: string) {
      const track = this.ambientTracks.find(t => t.id === id)
      if (track) track.paused = !track.paused
    },

    /**
     * 移除指定环境音轨道（通过ID）
     */
    removeAmbientTrack(id: string) {
      this.ambientTracks = this.ambientTracks.filter(t => t.id !== id)
    },

    /**
     * 清除环境音轨道
     * 传入 targetSrc 时按文件名匹配清除指定轨道，否则清除全部
     */
    clearAmbientTracks(targetSrc?: string) {
      if (targetSrc) {
        // 按文件名匹配清除指定轨道
        this.ambientTracks = this.ambientTracks.filter(
          t => !t.src.endsWith(targetSrc) && !t.src.includes(targetSrc)
        )
      } else {
        this.ambientTracks = []
      }
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

  // 全局唯一 resize 监听：更新视口尺寸供所有组件复用
  window.addEventListener('resize', () => {
    store.viewportWidth = window.innerWidth
    store.viewportHeight = window.innerHeight
  })

  const settingsStore = useSettingsStore()
  // 使用 getter 获取角色文件夹
  store.loadCharacterTips(store.currentCharacterFolder)
}

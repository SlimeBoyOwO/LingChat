/**
 * 统一设置管理 Store
 * 集中管理所有用户偏好设置，自动持久化到 localStorage
 */
import { setCurrentBackground } from '@/api/services/background'
import { setSceneAwareness } from '@/api/services/scene'
import { defineStore } from 'pinia'

// 默认设置值
export const DEFAULT_SETTINGS = {
  // 文本设置
  text: {
    speed: 80, // 打字速度 (0-100)
    animation: true, // 页面切换动画
    inlineMotionText: false, // 内联动作文本（单次显示台词+灰字动作）
    sedentaryReminder: false, // 久坐喝水提醒
  },
  // 音频设置
  audio: {
    characterVolume: 80, // 角色音量
    bubbleVolume: 80, // 气泡音量
    backgroundVolume: 80, // 背景音量
    achievementVolume: 80, // 成就音量
    ambientVolume: 70, // 环境音音量
    chatEffectSound: true, // 对话音效开关
  },
  // 显示设置
  display: {
    currentBackground: '@/assets/images/default_bg.jpg', // 当前背景图片
    backgroundEffect: 'StarField', // 背景效果名称
    mainMenuStarsEnabled: true, // 主菜单星星粒子开关
    mainMenuMeteorsEnabled: true, // 主菜单流星开关
    globalMouseTrailEnabled: true, // 全局鼠标滑动动画开关
    clickAnimationEnabled: true, // 点击动画开关
    meteorFps: 30, // 流星动画帧率
    starsFps: 30, // 星星动画帧率
    sceneAwarenessEnabled: true, // 场景感知开关
    // ── 对话框外观（自定义） ──
    dialogBackgroundImage: null as string | null, // 自定义背景图文件名（存于 data/backgrounds/）
    dialogOpacity: 70, // 对话框背景不透明度 0-100
    dialogBlur: 2, // 对话框背景模糊度 0-20 px
    dialogGradientColor: '#000e27', // 对话框渐变底色（HEX）
    dialogBorderRadius: 8, // 对话框圆角 0-32 px
    dialogTextColor: '#ffffff', // 对话框文字色（HEX）
    // ── 对话框交互行为 ──
    dialogScrollHistoryEnabled: true, // 鼠标滚轮查看历史记录
    dialogSpacebarHideEnabled: true, // 空格键隐藏/显示对话框
    dialogAutoHideOnThinkEnabled: true, // AI 思考时自动隐藏对话框
  },
  // 角色设置
  character: {
    folder: '诺一钦灵', // 当前角色文件夹
  },
  // 桌宠设置
  pet: {
    scale: 1, // 桌宠缩放比例
  },
}

// 设置状态类型
export interface TextSettings {
  speed: number
  animation: boolean
  inlineMotionText: boolean
  sedentaryReminder: boolean
}
export interface AudioSettings {
  characterVolume: number
  bubbleVolume: number
  backgroundVolume: number
  achievementVolume: number
  ambientVolume: number
  chatEffectSound: boolean
}
export interface DisplaySettings {
  currentBackground: string
  backgroundEffect: string
  mainMenuStarsEnabled: boolean
  mainMenuMeteorsEnabled: boolean
  globalMouseTrailEnabled: boolean
  clickAnimationEnabled: boolean
  meteorFps: number
  starsFps: number
  sceneAwarenessEnabled: boolean
  // ── 对话框外观 ──
  dialogBackgroundImage: string | null
  dialogOpacity: number
  dialogBlur: number
  dialogGradientColor: string
  dialogBorderRadius: number
  dialogTextColor: string
  // ── 对话框交互行为 ──
  dialogScrollHistoryEnabled: boolean
  dialogSpacebarHideEnabled: boolean
  dialogAutoHideOnThinkEnabled: boolean
}

export interface CharacterSettings {
  folder: string
}

export interface PetSettings {
  scale: number
}

export interface SettingsState {
  text: TextSettings
  audio: AudioSettings
  display: DisplaySettings
  character: CharacterSettings
  pet: PetSettings
}

export const useSettingsStore = defineStore('settings', {
  state: (): SettingsState => ({
    text: { ...DEFAULT_SETTINGS.text },
    audio: { ...DEFAULT_SETTINGS.audio },
    display: { ...DEFAULT_SETTINGS.display },
    character: { ...DEFAULT_SETTINGS.character },
    pet: { ...DEFAULT_SETTINGS.pet },
  }),

  getters: {
    // 获取设置值（支持路径）
    get:
      (state) =>
      (path: string): unknown => {
        return path.split('.').reduce<unknown>((obj, key) => {
          if (obj && typeof obj === 'object' && key in obj) {
            return (obj as Record<string, unknown>)[key]
          }
          return undefined
        }, state)
      },

    // 文字速度
    textSpeed: (state) => state.text.speed,
    // 对话音效开关
    chatEffectSound: (state) => state.audio.chatEffectSound,
    // 背景效果
    currentBackground: (state) => state.display.currentBackground,
    backgroundEffect: (state) => state.display.backgroundEffect,
    mainMenuStarsEnabled: (state) => state.display.mainMenuStarsEnabled,
    mainMenuMeteorsEnabled: (state) => state.display.mainMenuMeteorsEnabled,
    globalMouseTrailEnabled: (state) => state.display.globalMouseTrailEnabled,
    clickAnimationEnabled: (state) => state.display.clickAnimationEnabled,
    meteorFps: (state) => state.display.meteorFps,
    starsFps: (state) => state.display.starsFps,
    sceneAwarenessEnabled: (state) => state.display.sceneAwarenessEnabled,
    // ── 对话框外观 getter ──
    dialogBackgroundImage: (state) => state.display.dialogBackgroundImage,
    dialogOpacity: (state) => state.display.dialogOpacity,
    dialogBlur: (state) => state.display.dialogBlur,
    dialogGradientColor: (state) => state.display.dialogGradientColor,
    dialogBorderRadius: (state) => state.display.dialogBorderRadius,
    dialogTextColor: (state) => state.display.dialogTextColor,
    // ── 对话框交互行为 getter ──
    dialogScrollHistoryEnabled: (state) => state.display.dialogScrollHistoryEnabled,
    dialogSpacebarHideEnabled: (state) => state.display.dialogSpacebarHideEnabled,
    dialogAutoHideOnThinkEnabled: (state) => state.display.dialogAutoHideOnThinkEnabled,
    // 各音量
    characterVolume: (state) => state.audio.characterVolume,
    bubbleVolume: (state) => state.audio.bubbleVolume,
    backgroundVolume: (state) => state.audio.backgroundVolume,
    achievementVolume: (state) => state.audio.achievementVolume,
    ambientVolume: (state) => state.audio.ambientVolume,
    // 角色文件夹
    characterFolder: (state) => state.character.folder,
  },

  actions: {
    // 更新设置值（支持路径）
    update(path: string, value: unknown) {
      const keys = path.split('.')
      if (keys.length < 2) {
        console.warn(`无效的设置路径: ${path}`)
        return
      }

      let target: Record<string, unknown> = this as unknown as Record<string, unknown>
      for (let i = 0; i < keys.length - 1; i++) {
        const key = keys[i]
        if (!key || target[key] === undefined) {
          console.warn(`设置路径不存在: ${path}`)
          return
        }
        if (key) {
          target = target[key] as Record<string, unknown>
        }
      }

      const lastKey = keys[keys.length - 1]
      // 兼容新字段：即使持久化数据中不存在该字段也允许写入
      if (lastKey) {
        target[lastKey] = value
      }
    },

    // 重置设置
    reset(path?: string) {
      if (!path) {
        // 重置全部
        this.text = { ...DEFAULT_SETTINGS.text }
        this.audio = { ...DEFAULT_SETTINGS.audio }
        this.display = { ...DEFAULT_SETTINGS.display }
      } else {
        const keys = path.split('.')
        if (keys.length === 1) {
          // 重置整个分类
          const category = keys[0] as keyof SettingsState
          if (category in DEFAULT_SETTINGS) {
            this[category] = { ...DEFAULT_SETTINGS[category] } as never
          }
        } else {
          // 重置单个值
          const defaultValue = keys.reduce<unknown>((obj, key) => {
            if (obj && typeof obj === 'object' && key in obj) {
              return (obj as Record<string, unknown>)[key]
            }
            return undefined
          }, DEFAULT_SETTINGS as unknown)

          if (defaultValue !== undefined) {
            this.update(path, defaultValue)
          }
        }
      }
    },

    // 导出设置为 JSON 字符串
    exportSettings(): string {
      return JSON.stringify(this.$state, null, 2)
    },

    // 从 JSON 字符串导入设置
    importSettings(json: string): boolean {
      try {
        const data = JSON.parse(json)
        // 只导入有效的设置项
        if (data.text) this.text = { ...DEFAULT_SETTINGS.text, ...data.text }
        if (data.audio) this.audio = { ...DEFAULT_SETTINGS.audio, ...data.audio }
        if (data.display) this.display = { ...DEFAULT_SETTINGS.display, ...data.display }
        if (data.character) this.character = { ...DEFAULT_SETTINGS.character, ...data.character }
        if (data.pet) this.pet = { ...DEFAULT_SETTINGS.pet, ...data.pet }
        return true
      } catch (e) {
        console.error('导入设置失败:', e)
        return false
      }
    },

    // 批量更新音频设置
    updateAudio(updates: Partial<AudioSettings>) {
      this.audio = { ...this.audio, ...updates }
    },

    // 批量更新文本设置
    updateText(updates: Partial<TextSettings>) {
      this.text = { ...this.text, ...updates }
    },

    // 批量更新显示设置
    updateDisplay(updates: Partial<DisplaySettings>) {
      this.display = { ...this.display, ...updates }
    },

    // 设置文字速度
    setTextSpeed(speed: number) {
      this.text.speed = speed
    },

    setCurrentBackground(background: string) {
      this.display.currentBackground = background
    },

    // 设置对话音效开关
    setChatEffectSound(enabled: boolean) {
      this.audio.chatEffectSound = enabled
    },

    // 设置背景效果
    setBackgroundEffect(effect: string) {
      this.display.backgroundEffect = effect
    },
    // 设置主菜单星星粒子开关
    setMainMenuStarsEnabled(enabled: boolean) {
      this.display.mainMenuStarsEnabled = enabled
    },
    // 设置主菜单流星开关
    setMainMenuMeteorsEnabled(enabled: boolean) {
      this.display.mainMenuMeteorsEnabled = enabled
    },
    // 设置全局鼠标滑动动画开关
    setGlobalMouseTrailEnabled(enabled: boolean) {
      this.display.globalMouseTrailEnabled = enabled
    },
    // 设置点击动画开关
    setClickAnimationEnabled(enabled: boolean) {
      this.display.clickAnimationEnabled = enabled
    },

    // 设置流星动画帧率
    setMeteorFps(fps: number) {
      this.display.meteorFps = fps
    },

    // 设置星星动画帧率
    setStarsFps(fps: number) {
      this.display.starsFps = fps
    },

    // 设置场景感知开关（同步到后端）
    setSceneAwarenessEnabled(enabled: boolean) {
      this.display.sceneAwarenessEnabled = enabled
      setSceneAwareness(enabled)
    },

    // 设置角色文件夹
    setCharacterFolder(folder: string) {
      this.character.folder = folder
    },

    // 设置桌宠缩放比例
    setPetScale(scale: number) {
      if (!this.pet) {
        this.pet = { scale: 1.0 }
      }
      this.pet.scale = scale
    },

    // ── 对话框外观 setter ──
    setDialogBackgroundImage(filename: string | null) {
      this.display.dialogBackgroundImage = filename
    },
    setDialogOpacity(opacity: number) {
      this.display.dialogOpacity = Math.max(0, Math.min(100, opacity))
    },
    setDialogBlur(blur: number) {
      this.display.dialogBlur = Math.max(0, Math.min(20, blur))
    },
    setDialogGradientColor(color: string) {
      this.display.dialogGradientColor = color
    },
    setDialogBorderRadius(radius: number) {
      this.display.dialogBorderRadius = Math.max(0, Math.min(32, radius))
    },
    setDialogTextColor(color: string) {
      this.display.dialogTextColor = color
    },
    // ── 对话框交互行为 setter ──
    setDialogScrollHistoryEnabled(enabled: boolean) {
      this.display.dialogScrollHistoryEnabled = enabled
    },
    setDialogSpacebarHideEnabled(enabled: boolean) {
      this.display.dialogSpacebarHideEnabled = enabled
    },
    setDialogAutoHideOnThinkEnabled(enabled: boolean) {
      this.display.dialogAutoHideOnThinkEnabled = enabled
    },
  },

  // 启用持久化
  persist: true,
})

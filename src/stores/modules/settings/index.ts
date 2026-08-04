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
    fontFamily: '', // 自定义界面字体名（为空走系统默认栈；初始菜单/加载页不受影响）
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
    locale: 'zh-CN', // 界面显示语言（i18n，'zh-CN' / 'ja'）
    // 对话框外观（自定义）
    dialogBackgroundImage: '', // 自定义背景图 base64/dataURL；空字符串=无图
    dialogOpacity: 0.7, // 背景透明度（0-1）
    dialogBlur: 8, // 背景模糊（px）
    dialogBorderRadius: 16, // 圆角（px）
    dialogGradientColor: '#000e27', // 渐变底色
    dialogTextColor: '#ffffff', // 文字颜色
    dialogScrollHistoryEnabled: true, // 滚轮向上查看历史记录
    dialogSpacebarHideEnabled: true, // 空格键隐藏/显示对话框
    dialogAutoHideOnThinkEnabled: true, // AI 思考时自动隐藏
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
  fontFamily: string
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
    locale: string
    // 对话框外观
    dialogBackgroundImage: string
    dialogOpacity: number
    dialogBlur: number
    dialogBorderRadius: number
    dialogGradientColor: string
    dialogTextColor: string
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
    // 界面显示语言（i18n）
    uiLocale: (state) => state.display.locale,
    // 对话框外观
    dialogBackgroundImage: (state) => state.display.dialogBackgroundImage,
    dialogOpacity: (state) => state.display.dialogOpacity,
    dialogBlur: (state) => state.display.dialogBlur,
    dialogBorderRadius: (state) => state.display.dialogBorderRadius,
    dialogGradientColor: (state) => state.display.dialogGradientColor,
    dialogTextColor: (state) => state.display.dialogTextColor,
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

    // 设置界面显示语言（i18n）
    setUiLocale(locale: string) {
      this.display.locale = locale
    },

    // ===== 对话框外观 =====
    setDialogBackgroundImage(image: string) {
      this.display.dialogBackgroundImage = image
    },
    setDialogOpacity(opacity: number) {
      this.display.dialogOpacity = Math.min(1, Math.max(0, opacity))
    },
    setDialogBlur(blur: number) {
      this.display.dialogBlur = Math.max(0, blur)
    },
    setDialogBorderRadius(radius: number) {
      this.display.dialogBorderRadius = Math.max(0, radius)
    },
    setDialogGradientColor(color: string) {
      this.display.dialogGradientColor = color
    },
    setDialogTextColor(color: string) {
      this.display.dialogTextColor = color
    },
    setDialogScrollHistoryEnabled(enabled: boolean) {
      this.display.dialogScrollHistoryEnabled = enabled
    },
    setDialogSpacebarHideEnabled(enabled: boolean) {
      this.display.dialogSpacebarHideEnabled = enabled
    },
    setDialogAutoHideOnThinkEnabled(enabled: boolean) {
      this.display.dialogAutoHideOnThinkEnabled = enabled
    },
    // 全部重置为默认
    resetDialogAppearance() {
      const d = DEFAULT_SETTINGS.display
      this.display.dialogBackgroundImage = d.dialogBackgroundImage
      this.display.dialogOpacity = d.dialogOpacity
      this.display.dialogBlur = d.dialogBlur
      this.display.dialogBorderRadius = d.dialogBorderRadius
      this.display.dialogGradientColor = d.dialogGradientColor
      this.display.dialogTextColor = d.dialogTextColor
      this.display.dialogScrollHistoryEnabled = d.dialogScrollHistoryEnabled
      this.display.dialogSpacebarHideEnabled = d.dialogSpacebarHideEnabled
      this.display.dialogAutoHideOnThinkEnabled = d.dialogAutoHideOnThinkEnabled
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
  },

  // 启用持久化
  persist: true,
})

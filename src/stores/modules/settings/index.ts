/**
 * 统一设置管理 Store
 * 集中管理所有用户偏好设置，自动持久化到 localStorage
 */
import { setHdrMode } from "@/api/services/config";
import { setSceneAwareness } from "@/api/services/scene";
import {
  DEFAULT_SPECTRUM_COLOR_FROM,
  DEFAULT_SPECTRUM_COLOR_TO,
  DEFAULT_SPECTRUM_PALETTE,
  DEFAULT_SPECTRUM_STYLE,
  type SpectrumStyle,
} from "@/constants/spectrum";
import type { ShortcutAction, ShortcutBinding } from "@/utils/shortcuts";
import { DEFAULT_SHORTCUTS, sanitizeShortcuts } from "@/utils/shortcuts";
import { isWeatherEffect } from "@/components/game/standard/particles";
import { defineStore } from "pinia";

// 默认设置值
export const DEFAULT_SETTINGS = {
  // 文本设置
  text: {
    speed: 80, // 打字速度 (0-100)
    autoAdvanceDelay: 1000, // 自动模式自动推进延迟 (ms, 0-2000)
    animation: true, // 页面切换动画
    inlineMotionText: false, // 内联动作文本（单次显示台词+灰字动作）
    mergeLineThreshold: 55, // 台词合并阈值：同角色连续短句（字符数）自动合并续打；0=关闭
    mergeLineDelay: 200, // 台词合并时上一句展示完成到续打的延迟 (ms)
    mergeMotionMode: "append" as const, // 台词合并时动作文本的处理方式：append=接在后面显示（| 分隔）/ replace=清空旧动作，独立显示本次动作
    sedentaryReminder: false, // 久坐喝水提醒
    fontFamily: "", // 自定义界面字体名（为空走系统默认栈；初始菜单/加载页不受影响）
    vueDevToolsEnabled: true, // Vue DevTools 悬浮面板显示开关（仅开发模式生效，仅主窗口）
  },
  // 音频设置
  audio: {
    characterVolume: 80, // 角色音量
    bubbleVolume: 80, // 气泡音量
    backgroundVolume: 80, // 背景音量
    achievementVolume: 80, // 成就音量
    ambientVolume: 70, // 环境音音量
    chatEffectSound: true, // 对话音效开关
    outputDeviceId: "", // 输出音频设备（'' = 跟随系统默认）
    // 音频频谱可视化（右下角迷你频谱）：默认关闭——开启后音频会接入 Web Audio
    // 图（见 utils/audioSpectrum.ts），不想要的用户不该被默认卷进来。
    spectrumEnabled: false, // 频谱可视化开关
    spectrumStyle: DEFAULT_SPECTRUM_STYLE, // 形态：mirror 镜像 / bars 柱状 / ring 圆环
    spectrumPalette: DEFAULT_SPECTRUM_PALETTE, // 配色方案 id（见 constants/spectrum.ts）
    spectrumColor1: DEFAULT_SPECTRUM_COLOR_FROM, // 自定义配色：主色
    spectrumColor2: DEFAULT_SPECTRUM_COLOR_TO, // 自定义配色：辅色
  },
  // 显示设置
  display: {
    currentBackground: "@/assets/images/default_bg.jpg", // 当前背景图片
    backgroundEffect: "StarField", // 氛围特效名称
    weatherEffect: "None", // 天气特效名称，与氛围特效相互独立
    mainMenuStarsEnabled: true, // 主菜单星星粒子开关
    mainMenuMeteorsEnabled: true, // 主菜单流星开关
    globalMouseTrailEnabled: true, // 全局鼠标滑动动画开关
    clickAnimationEnabled: true, // 点击动画开关
    cursorEffectEngine: "ba-click-fx" as CursorEffectEngine, // 光标特效引擎：新版 WebGL2 / 旧 Canvas2D 实现
    meteorFps: 30, // 流星动画帧率
    starsFps: 30, // 星星动画帧率
    sceneAwarenessEnabled: true, // 场景感知开关
    hdrModeEnabled: false, // HDR 模式开关（仅 Windows）
    locale: "zh-CN", // 界面显示语言（i18n，'zh-CN' / 'ja'）
    affectionHeartbeatEnabled: true, // 好感度爱心心跳动画开关（关闭后液体爱心静止）
    affectionWaveEnabled: true, // 好感度爱心液体波浪动画开关（关闭后液面为静止平面）
  },
  // 角色设置
  character: {
    folder: "诺一钦灵", // 当前角色文件夹
  },
  // 桌宠设置
  pet: {
    scale: 1, // 桌宠缩放比例
    live2dFps: 30, // Live2D 渲染帧率上限（0 = 不限制）；桌宠窗口小，30 帧足够且显著降 CPU
    bubbleSide: "above" as BubbleSide, // 气泡/通知位置：气泡在宠物的哪一侧（四向 + 自动）
  },
  // 剧本编辑器快捷键（默认不含 Command 键；可在编辑器快捷键面板自定义）
  shortcuts: DEFAULT_SHORTCUTS,
};

// 设置状态类型
export interface TextSettings {
  speed: number;
  autoAdvanceDelay: number;
  animation: boolean;
  inlineMotionText: boolean;
  mergeLineThreshold: number;
  mergeLineDelay: number;
  mergeMotionMode: "append" | "replace";
  sedentaryReminder: boolean;
  fontFamily: string;
  vueDevToolsEnabled: boolean;
}
export interface AudioSettings {
  characterVolume: number;
  bubbleVolume: number;
  backgroundVolume: number;
  achievementVolume: number;
  ambientVolume: number;
  chatEffectSound: boolean;
  outputDeviceId: string;
  /** 音频频谱可视化开关 */
  spectrumEnabled: boolean;
  /** 频谱形态：mirror 镜像 / bars 柱状 / ring 圆环 */
  spectrumStyle: SpectrumStyle;
  /** 频谱配色方案 id（"custom" = 用下面两个自定义色） */
  spectrumPalette: string;
  /** 自定义配色：主色 / 辅色 */
  spectrumColor1: string;
  spectrumColor2: string;
}
/**
 * 光标特效引擎。
 *
 * `ba-click-fx` = 第三方库实现（WebGL2，性能更好，默认）；
 * `legacy` = 内置的 Canvas2D 实现（src/components/effects/CursorEffects.vue）。
 */
export type CursorEffectEngine = "ba-click-fx" | "legacy";

export interface DisplaySettings {
  currentBackground: string;
  backgroundEffect: string;
  weatherEffect: string;
  mainMenuStarsEnabled: boolean;
  mainMenuMeteorsEnabled: boolean;
  globalMouseTrailEnabled: boolean;
  clickAnimationEnabled: boolean;
  cursorEffectEngine: CursorEffectEngine;
  meteorFps: number;
  starsFps: number;
  sceneAwarenessEnabled: boolean;
  hdrModeEnabled: boolean;
  locale: string;
  affectionHeartbeatEnabled: boolean;
  affectionWaveEnabled: boolean;
}

export interface CharacterSettings {
  folder: string;
}

/**
 * 气泡/通知位置：气泡放在宠物的哪一侧。
 *
 * `above` / `below` / `left` / `right` = 手动指定；`auto` = 由宠物窗按宠物在屏幕上的
 * 位置与工作区余量自动选边（优先上、下，再右、左；都放不下时选余量最大的一边）。
 */
export type BubbleSide = "above" | "below" | "left" | "right" | "auto";

export interface PetSettings {
  scale: number;
  /** Live2D 渲染帧率上限（0 = 不限制），默认 30 */
  live2dFps: number;
  /** 气泡/通知位置 */
  bubbleSide: BubbleSide;
}

export interface SettingsState {
  text: TextSettings;
  audio: AudioSettings;
  display: DisplaySettings;
  character: CharacterSettings;
  pet: PetSettings;
  shortcuts: Record<ShortcutAction, ShortcutBinding>;
}

export const useSettingsStore = defineStore("settings", {
  state: (): SettingsState => ({
    text: { ...DEFAULT_SETTINGS.text },
    audio: { ...DEFAULT_SETTINGS.audio },
    display: { ...DEFAULT_SETTINGS.display },
    character: { ...DEFAULT_SETTINGS.character },
    pet: { ...DEFAULT_SETTINGS.pet },
    shortcuts: { ...DEFAULT_SETTINGS.shortcuts },
  }),

  getters: {
    // 获取设置值（支持路径）
    get:
      (state) =>
      (path: string): unknown => {
        return path.split(".").reduce<unknown>((obj, key) => {
          if (obj && typeof obj === "object" && key in obj) {
            return (obj as Record<string, unknown>)[key];
          }
          return undefined;
        }, state);
      },

    // 文字速度
    textSpeed: (state) => state.text.speed,
    // 自动模式自动推进延迟 (ms)
    autoAdvanceDelay: (state) => state.text.autoAdvanceDelay,
    // 对话音效开关
    chatEffectSound: (state) => state.audio.chatEffectSound,
    // 背景效果
    currentBackground: (state) => state.display.currentBackground,
    backgroundEffect: (state) => state.display.backgroundEffect,
    weatherEffect: (state) => state.display.weatherEffect,
    mainMenuStarsEnabled: (state) => state.display.mainMenuStarsEnabled,
    mainMenuMeteorsEnabled: (state) => state.display.mainMenuMeteorsEnabled,
    globalMouseTrailEnabled: (state) => state.display.globalMouseTrailEnabled,
    clickAnimationEnabled: (state) => state.display.clickAnimationEnabled,
    // 光标特效引擎（旧持久化数据缺该字段时回退新版）
    cursorEffectEngine: (state): CursorEffectEngine =>
      state.display.cursorEffectEngine ?? "ba-click-fx",
    meteorFps: (state) => state.display.meteorFps,
    starsFps: (state) => state.display.starsFps,
    sceneAwarenessEnabled: (state) => state.display.sceneAwarenessEnabled,
    // HDR 模式开关（仅 Windows）
    hdrModeEnabled: (state) => state.display.hdrModeEnabled,
    // 界面显示语言（i18n）
    uiLocale: (state) => state.display.locale,
    // 好感度爱心心跳动画开关（旧持久化数据缺该字段时回退 true）
    affectionHeartbeatEnabled: (state) => state.display.affectionHeartbeatEnabled ?? true,
    // 好感度爱心液体波浪动画开关（同上回退 true）
    affectionWaveEnabled: (state) => state.display.affectionWaveEnabled ?? true,
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
    // 校验快捷键数据：旧版本捕获逻辑可能写入非法绑定（如把 Ctrl+S 绑成单独的 S），
    // 非法项回退默认。编辑器挂载时调用一次，幂等。
    ensureValidShortcuts() {
      this.shortcuts = sanitizeShortcuts(this.shortcuts);
    },

    // 更新设置值（支持路径）
    update(path: string, value: unknown) {
      const keys = path.split(".");
      if (keys.length < 2) {
        console.warn(`无效的设置路径: ${path}`);
        return;
      }

      let target: Record<string, unknown> = this as unknown as Record<string, unknown>;
      for (let i = 0; i < keys.length - 1; i++) {
        const key = keys[i];
        if (!key || target[key] === undefined) {
          console.warn(`设置路径不存在: ${path}`);
          return;
        }
        if (key) {
          target = target[key] as Record<string, unknown>;
        }
      }

      const lastKey = keys[keys.length - 1];
      // 兼容新字段：即使持久化数据中不存在该字段也允许写入
      if (lastKey) {
        target[lastKey] = value;
      }
    },

    // 重置设置
    reset(path?: string) {
      if (!path) {
        // 重置全部
        this.text = { ...DEFAULT_SETTINGS.text };
        this.audio = { ...DEFAULT_SETTINGS.audio };
        this.display = { ...DEFAULT_SETTINGS.display };
        this.shortcuts = { ...DEFAULT_SETTINGS.shortcuts };
      } else {
        const keys = path.split(".");
        if (keys.length === 1) {
          // 重置整个分类
          const category = keys[0] as keyof SettingsState;
          if (category in DEFAULT_SETTINGS) {
            this[category] = { ...DEFAULT_SETTINGS[category] } as never;
          }
        } else {
          // 重置单个值
          const defaultValue = keys.reduce<unknown>((obj, key) => {
            if (obj && typeof obj === "object" && key in obj) {
              return (obj as Record<string, unknown>)[key];
            }
            return undefined;
          }, DEFAULT_SETTINGS as unknown);

          if (defaultValue !== undefined) {
            this.update(path, defaultValue);
          }
        }
      }
    },

    // 导出设置为 JSON 字符串
    exportSettings(): string {
      return JSON.stringify(this.$state, null, 2);
    },

    // 从 JSON 字符串导入设置
    importSettings(json: string): boolean {
      try {
        const data = JSON.parse(json);
        // 只导入有效的设置项
        if (data.text) this.text = { ...DEFAULT_SETTINGS.text, ...data.text };
        if (data.audio) this.audio = { ...DEFAULT_SETTINGS.audio, ...data.audio };
        if (data.display) this.display = { ...DEFAULT_SETTINGS.display, ...data.display };
        if (data.character) this.character = { ...DEFAULT_SETTINGS.character, ...data.character };
        if (data.pet) this.pet = { ...DEFAULT_SETTINGS.pet, ...data.pet };
        if (data.shortcuts) this.shortcuts = { ...DEFAULT_SETTINGS.shortcuts, ...data.shortcuts };
        return true;
      } catch (e) {
        console.error("导入设置失败:", e);
        return false;
      }
    },

    // 批量更新音频设置
    updateAudio(updates: Partial<AudioSettings>) {
      this.audio = { ...this.audio, ...updates };
    },

    // 批量更新文本设置
    updateText(updates: Partial<TextSettings>) {
      this.text = { ...this.text, ...updates };
    },

    // 批量更新显示设置
    updateDisplay(updates: Partial<DisplaySettings>) {
      this.display = { ...this.display, ...updates };
    },

    // 设置文字速度
    setTextSpeed(speed: number) {
      this.text.speed = speed;
    },

    setCurrentBackground(background: string) {
      this.display.currentBackground = background;
    },

    // 设置对话音效开关
    setChatEffectSound(enabled: boolean) {
      this.audio.chatEffectSound = enabled;
    },

    // 设置背景效果
    setBackgroundEffect(effect: string) {
      this.display.backgroundEffect = effect;
    },

    // 设置天气效果
    setWeatherEffect(effect: string) {
      this.display.weatherEffect = effect;
    },
    // 设置主菜单星星粒子开关
    setMainMenuStarsEnabled(enabled: boolean) {
      this.display.mainMenuStarsEnabled = enabled;
    },
    // 设置主菜单流星开关
    setMainMenuMeteorsEnabled(enabled: boolean) {
      this.display.mainMenuMeteorsEnabled = enabled;
    },
    // 设置全局鼠标滑动动画开关
    setGlobalMouseTrailEnabled(enabled: boolean) {
      this.display.globalMouseTrailEnabled = enabled;
    },
    // 设置点击动画开关
    setClickAnimationEnabled(enabled: boolean) {
      this.display.clickAnimationEnabled = enabled;
    },
    // 设置光标特效引擎
    setCursorEffectEngine(engine: CursorEffectEngine) {
      this.display.cursorEffectEngine = engine;
    },

    // 设置流星动画帧率
    setMeteorFps(fps: number) {
      this.display.meteorFps = fps;
    },

    // 设置星星动画帧率
    setStarsFps(fps: number) {
      this.display.starsFps = fps;
    },

    // 设置场景感知开关（同步到后端）
    setSceneAwarenessEnabled(enabled: boolean) {
      this.display.sceneAwarenessEnabled = enabled;
      setSceneAwareness(enabled);
    },

    // 设置 HDR 模式开关（仅 Windows；同步到后端，重启后生效）
    setHdrModeEnabled(enabled: boolean) {
      this.display.hdrModeEnabled = enabled;
      setHdrMode(enabled);
    },

    // 设置界面显示语言（i18n）
    setUiLocale(locale: string) {
      this.display.locale = locale;
    },

    // 设置好感度爱心心跳动画开关
    setAffectionHeartbeatEnabled(enabled: boolean) {
      this.display.affectionHeartbeatEnabled = enabled;
    },
    // 设置好感度爱心液体波浪动画开关
    setAffectionWaveEnabled(enabled: boolean) {
      this.display.affectionWaveEnabled = enabled;
    },
    // 设置角色文件夹
    setCharacterFolder(folder: string) {
      this.character.folder = folder;
    },

    // 设置桌宠缩放比例
    setPetScale(scale: number) {
      if (!this.pet) {
        this.pet = { ...DEFAULT_SETTINGS.pet };
      }
      this.pet.scale = scale;
    },

    // 设置 Live2D 渲染帧率上限（0 = 不限制）
    setPetLive2dFps(fps: number) {
      if (!this.pet) {
        this.pet = { ...DEFAULT_SETTINGS.pet };
      }
      this.pet.live2dFps = fps;
    },
  },

  // 启用持久化
  persist: {
    // 雨从氛围层挪到天气层之后，老配置里的 backgroundEffect: "Rain" 就指向了一个
    // 氛围层已经没有的 key —— 不报错，但什么也不显示。这里把它归位。
    // 写成不变量而不是一次性迁移：任何来源的越界值都会在读取时被纠正
    afterHydrate: (store: SettingsState) => {
      if (!isWeatherEffect(store.display.backgroundEffect)) return;
      store.display.weatherEffect = store.display.backgroundEffect;
      store.display.backgroundEffect = "None";
    },
  },
});

/**
 * 统一设置管理 Store
 * 集中管理所有用户偏好设置，自动持久化到 localStorage
 */
import { defineStore } from "pinia";

const PET_SCALE_STORAGE_KEY = "lingchat.petScale";
const SETTINGS_STORAGE_KEY = "lingchat.settings";
export const PET_SCALE_MIN = 0.7;
export const PET_SCALE_MAX = 1.3;
export const PET_SCALE_DEFAULT = 1;

function clampPetScale(scale: number): number {
  if (Number.isNaN(scale)) {
    return PET_SCALE_DEFAULT;
  }
  return Math.min(PET_SCALE_MAX, Math.max(PET_SCALE_MIN, scale));
}

function readPetScaleFromStorage(): number {
  if (typeof window === "undefined") {
    return PET_SCALE_DEFAULT;
  }

  const raw = window.localStorage.getItem(PET_SCALE_STORAGE_KEY);
  if (!raw) {
    return PET_SCALE_DEFAULT;
  }

  const parsed = Number(raw);
  if (Number.isNaN(parsed)) {
    return PET_SCALE_DEFAULT;
  }

  return clampPetScale(parsed);
}

function readSettingsFromStorage(): Partial<SettingsState> {
  if (typeof window === "undefined") {
    return {};
  }

  const raw = window.localStorage.getItem(SETTINGS_STORAGE_KEY);
  if (!raw) {
    return {};
  }

  try {
    const parsed = JSON.parse(raw);
    return parsed;
  } catch (e) {
    console.error("读取设置失败:", e);
    return {};
  }
}

function saveSettingsToStorage(settings: SettingsState): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
  } catch (e) {
    console.error("保存设置失败:", e);
  }
}

// 默认设置值
export const DEFAULT_SETTINGS = {
  // 文本设置
  text: {
    speed: 50, // 打字速度 (0-100)
    animation: true, // 页面切换动画
  },
  // 音频设置
  audio: {
    characterVolume: 80, // 角色音量
    bubbleVolume: 80, // 气泡音量
    backgroundVolume: 80, // 背景音量
    achievementVolume: 80, // 成就音量
    chatEffectSound: true, // 对话音效开关
  },
  // 显示设置
  display: {
    backgroundEffect: "BA", // 背景效果名称
  },
  // 角色设置
  character: {
    folder: "诺一钦灵", // 当前角色文件夹
  },
  pet: {
    scale: PET_SCALE_DEFAULT, // 桌宠缩放
  },
  isDarkMode: {
    mode: true,
  }, // 是否为暗黑模式
};

// 设置状态类型
export interface TextSettings {
  speed: number;
  animation: boolean;
}
export interface AudioSettings {
  characterVolume: number;
  bubbleVolume: number;
  backgroundVolume: number;
  achievementVolume: number;
  chatEffectSound: boolean;
}
export interface DisplaySettings {
  backgroundEffect: string;
}

export interface CharacterSettings {
  folder: string;
}

export interface PetSettings {
  scale: number;
}

export interface IsDarkModeSettings {
  mode: boolean;
}

export interface SettingsState {
  text: TextSettings;
  audio: AudioSettings;
  display: DisplaySettings;
  character: CharacterSettings;
  pet: PetSettings;
  isDarkMode: IsDarkModeSettings;
}

export const useSettingsStore = defineStore("settings", {
  state: (): SettingsState => {
    const savedSettings = readSettingsFromStorage();
    return {
      text: { ...DEFAULT_SETTINGS.text, ...savedSettings.text },
      audio: { ...DEFAULT_SETTINGS.audio, ...savedSettings.audio },
      display: { ...DEFAULT_SETTINGS.display, ...savedSettings.display },
      character: { ...DEFAULT_SETTINGS.character, ...savedSettings.character },
      pet: {
        scale:
          savedSettings.pet?.scale !== undefined
            ? clampPetScale(savedSettings.pet.scale)
            : readPetScaleFromStorage(),
      },
      isDarkMode: {
        ...DEFAULT_SETTINGS.isDarkMode,
        ...savedSettings.isDarkMode,
      },
    };
  },

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
    // 对话音效开关
    chatEffectSound: (state) => state.audio.chatEffectSound,
    // 背景效果
    backgroundEffect: (state) => state.display.backgroundEffect,
    // 各音量
    characterVolume: (state) => state.audio.characterVolume,
    bubbleVolume: (state) => state.audio.bubbleVolume,
    backgroundVolume: (state) => state.audio.backgroundVolume,
    achievementVolume: (state) => state.audio.achievementVolume,
    // 角色文件夹
    characterFolder: (state) => state.character.folder,
    // 桌宠缩放
    petScale: (state) => state.pet.scale,
  },

  actions: {
    // 更新设置值（支持路径）
    update(path: string, value: unknown) {
      const keys = path.split(".");
      if (keys.length < 2) {
        console.warn(`无效的设置路径: ${path}`);
        return;
      }

      let target: Record<string, unknown> = this as unknown as Record<
        string,
        unknown
      >;
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
      if (lastKey && lastKey in target) {
        if (path === "pet.scale") {
          const nextScale = clampPetScale(Number(value));
          target[lastKey] = nextScale;
          this.persistPetScale(nextScale);
        } else {
          target[lastKey] = value;
        }
        // 保存所有设置到localStorage
        saveSettingsToStorage(this.$state);
      }
    },

    // 重置设置
    reset(path?: string) {
      if (!path) {
        // 重置全部
        this.text = { ...DEFAULT_SETTINGS.text };
        this.audio = { ...DEFAULT_SETTINGS.audio };
        this.display = { ...DEFAULT_SETTINGS.display };
        this.character = { ...DEFAULT_SETTINGS.character };
        this.pet = { ...DEFAULT_SETTINGS.pet };
        this.persistPetScale(this.pet.scale);
        saveSettingsToStorage(this.$state);
      } else {
        const keys = path.split(".");
        if (keys.length === 1) {
          // 重置整个分类
          const category = keys[0] as keyof SettingsState;
          if (category in DEFAULT_SETTINGS) {
            this[category] = { ...DEFAULT_SETTINGS[category] } as never;
            if (category === "pet") {
              this.persistPetScale(this.pet.scale);
            }
            saveSettingsToStorage(this.$state);
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
        if (data.audio)
          this.audio = { ...DEFAULT_SETTINGS.audio, ...data.audio };
        if (data.display)
          this.display = { ...DEFAULT_SETTINGS.display, ...data.display };
        if (data.character)
          this.character = { ...DEFAULT_SETTINGS.character, ...data.character };
        if (data.pet) {
          this.pet = {
            ...DEFAULT_SETTINGS.pet,
            ...data.pet,
            scale: clampPetScale(Number(data.pet.scale)),
          };
          this.persistPetScale(this.pet.scale);
        }
        saveSettingsToStorage(this.$state);
        return true;
      } catch (e) {
        console.error("导入设置失败:", e);
        return false;
      }
    },

    // 批量更新音频设置
    updateAudio(updates: Partial<AudioSettings>) {
      this.audio = { ...this.audio, ...updates };
      saveSettingsToStorage(this.$state);
    },

    // 批量更新文本设置
    updateText(updates: Partial<TextSettings>) {
      this.text = { ...this.text, ...updates };
      saveSettingsToStorage(this.$state);
    },

    // 批量更新显示设置
    updateDisplay(updates: Partial<DisplaySettings>) {
      this.display = { ...this.display, ...updates };
      saveSettingsToStorage(this.$state);
    },

    // 设置文字速度
    setTextSpeed(speed: number) {
      this.text.speed = speed;
      saveSettingsToStorage(this.$state);
    },

    // 设置对话音效开关
    setChatEffectSound(enabled: boolean) {
      this.audio.chatEffectSound = enabled;
      saveSettingsToStorage(this.$state);
    },

    // 设置背景效果
    setBackgroundEffect(effect: string) {
      this.display.backgroundEffect = effect;
      saveSettingsToStorage(this.$state);
    },

    // 设置角色文件夹
    setCharacterFolder(folder: string) {
      this.character.folder = folder;
      saveSettingsToStorage(this.$state);
    },

    setPetScale(scale: number) {
      const nextScale = clampPetScale(scale);
      this.pet.scale = nextScale;
      this.persistPetScale(nextScale);
      saveSettingsToStorage(this.$state);
    },

    resetPetScale() {
      this.setPetScale(PET_SCALE_DEFAULT);
    },

    setDarkMode(enabled: boolean) {
      this.isDarkMode = { mode: enabled };
      saveSettingsToStorage(this.$state);
    },

    persistPetScale(scale: number) {
      if (typeof window === "undefined") {
        return;
      }
      window.localStorage.setItem(PET_SCALE_STORAGE_KEY, String(scale));
    },
  },
});

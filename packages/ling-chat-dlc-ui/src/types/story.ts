import type { ScriptEvent, ScriptChoiceItem } from "@lingchat/host/types/script";
/** 突脸惊吓事件：全屏图片闪现 + 音效 */
export interface ScriptJumpscareEvent extends ScriptEvent {
  type: "jumpscare";
  imagePath: string;
  soundPath?: string;
}

/** 玩家可见时间轴上的显式停顿。 */
export interface ScriptWaitEvent extends ScriptEvent {
  type: "wait";
}

/** 队列有序的显式窗口标题意图；空串恢复默认标题。 */
export interface ScriptWindowTitleEvent extends ScriptEvent {
  type: "window_title";
  title: string;
}

/** Rust 已安全校验、等待在正确剧情位置显示的本地故障窗口票据。 */
export interface ScriptGlitchWindowEvent extends ScriptEvent {
  type: "glitch_window";
  requestId: number;
}

/** Rust 已校验且绑定当前剧本运行的一次性原生系统窗口票据。 */
export interface ScriptConsoleWindowEvent extends ScriptEvent {
  type: "console_window";
  requestId: number;
}

/** 必须与目标台词同处前端时间轴的语音变速/变调。 */
export interface ScriptVoiceShiftEvent extends ScriptEvent {
  type: "voice_shift";
  rate?: number;
  pitch?: number;
}

/** 强制选择事件：鼠标被拖向 forced 选项，最终只能提交它 */
export interface ScriptForceChoiceEvent extends ScriptEvent {
  type: "force_choice";
  requestId: string;
  choices: ScriptChoiceItem[];
  forced: string;
}

export interface ScriptPoemWord {
  text: string;
  warmPoints: number;
  scriptPoints: number;
  voidPoints: number;
  glitch: boolean;
}

/** 20 轮选词写诗互动；每轮包含 10 个词。 */
export interface ScriptPoemGameEvent extends ScriptEvent {
  type: "poem_game";
  requestId: string;
  backgroundPath: string;
  musicPath: string;
  glitchMusicPath: string;
  warmStickerPath: string;
  scriptStickerPath: string;
  voidStickerPath: string;
  mode: "normal" | "act2" | "act2_final";
  rounds: ScriptPoemWord[][];
  normalLoopStart: number;
  glitchLoopStart: number;
}

export type ExtensionScriptEvent =
  | ScriptJumpscareEvent
  | ScriptWaitEvent
  | ScriptWindowTitleEvent
  | ScriptGlitchWindowEvent
  | ScriptConsoleWindowEvent
  | ScriptVoiceShiftEvent
  | ScriptForceChoiceEvent
  | ScriptPoemGameEvent;

export interface BackgroundPresentation {
  /** BSOD 假异常窗口的 trace 行文本（剧本自带彩蛋，缺省用通用默认） */
  text?: string;
  /** BSOD 彩蛋独白（延迟淡入的小字），缺省不显示 */
  echo?: string;
}
export interface CharacterPresentation {
  /** true 时为"闪现"演出：情绪立绘短暂展示 duration 秒后自动还原，不写回角色状态 */
  flash?: boolean;
  /** 立绘噪点侵蚀预设（DDLC n_rects_ghost 式）：'eyes' / 'mouth' / 'eyes_mouth'；'none' 清除 */
  noise?: string;
  /** 噪点淡入秒数；0/未设置 = 立即全显 */
  noiseFadeIn?: number;
}

export interface ScriptEvent {
  type: string
  duration: number
  isFinal?: boolean
}

export interface ScriptChapterChangeEvent extends ScriptEvent {
  type: 'chapter_change'
  chapterName: string
}

export interface ScriptNarrationEvent extends ScriptEvent {
  type: 'narration'
  text: string
  displayName?: string
  sceneId?: string
}

export interface ScriptPlayerEvent extends ScriptEvent {
  type: 'player'
  text: string
  displayName?: string
  displaySubtitle?: string
  emotion?: string
}

export interface ScriptDialogueEvent extends ScriptEvent {
  type: 'reply'
  character?: string
  roleId: number
  emotion: string
  originalTag: string
  message: string
  motionText: string
  ttsText?: string
  audioFile?: string
  originalMessage: string
  displayName?: string
  displaySubtitle?: string
  /** 触发此回复的用户消息序号（1-indexed） */
  userMessageSeq?: number
  /** 本轮生成的思考链（仅最后一帧携带） */
  thinking?: string
}

export interface ScriptThinkingEvent extends ScriptEvent {
  type: 'thinking'
  isThinking: boolean
}

export interface ScriptFreeDialogueEvent extends ScriptEvent {
  type: 'free_dialogue'
  switch: boolean
  maxRounds: number
  endLine: string
}

export interface ScriptBackgroundEvent extends ScriptEvent {
  type: 'background'
  imagePath: string
  transition: number
}

export interface ScriptPresentPicEvent extends ScriptEvent {
  type: 'present_pic'
  imagePath: string
  scale: number
}

export interface ScriptBackgroundEffectEvent extends ScriptEvent {
  type: 'background_effect'
  effect: string
  /** BSOD 假异常窗口的 trace 行文本（剧本自带彩蛋，缺省用通用默认） */
  text?: string
  /** BSOD 彩蛋独白（延迟淡入的小字），缺省不显示 */
  echo?: string
}

export interface ScriptSoundEvent extends ScriptEvent {
  type: 'sound'
  soundPath: string
}

export interface ScriptMusicEvent extends ScriptEvent {
  type: 'music'
  musicPath: string
  /** 播放速度倍率（1.0 原速）；未设置时前端按 1.0 处理 */
  playbackSpeed?: number
}

/** 环境音事件 —— 循环持续的场景音效，与 BGM 共存 */
export interface ScriptAmbientEvent extends ScriptEvent {
  type: 'ambient'
  ambientPath: string
  /** 单轨音量 0-100，默认 100 */
  volume?: number
  /** 是否循环，默认 true */
  loop?: boolean
  /** 是否停止（true 时淡出停止），默认 false */
  stop?: boolean
  /** 是否启用淡入淡出，默认 true */
  fade?: boolean
}

export interface ScriptModifyCharacterEvent extends ScriptEvent {
  type: 'modify_character'
  characterId: number
  emotion?: string
  action?: string
  clothes?: string
  /** true 时为"闪现"演出：情绪立绘短暂展示 duration 秒后自动还原，不写回角色状态 */
  flash?: boolean
}

export interface ScriptInputEvent extends ScriptEvent {
  type: 'input'
  hint: string
}
export interface ScriptChoiceEvent extends ScriptEvent {
  type: 'choice'
  choices: ScriptChoiceItem[]
  allowFree: boolean
}
/** 单个选项。disabled 表示条件不满足不可选，reason 是作者写的锁定提示 */
export interface ScriptChoiceItem {
  text: string
  disabled: boolean
  reason?: string
}
export interface ScriptEndEvent extends ScriptEvent {
  type: 'script_end'
  /** false 表示剧本是因为出错被中止的，不应记为完成 */
  completed?: boolean
}

/** 突脸惊吓事件：全屏图片闪现 + 音效 */
export interface ScriptJumpscareEvent extends ScriptEvent {
  type: 'jumpscare'
  imagePath: string
  soundPath?: string
}

/** 玩家可见时间轴上的显式停顿。 */
export interface ScriptWaitEvent extends ScriptEvent {
  type: 'wait'
}

/** Rust 已安全校验、等待在正确剧情位置显示的本地故障窗口票据。 */
export interface ScriptGlitchWindowEvent extends ScriptEvent {
  type: 'glitch_window'
  requestId: number
}

/** Rust 已校验且绑定当前剧本运行的一次性原生系统窗口票据。 */
export interface ScriptConsoleWindowEvent extends ScriptEvent {
  type: 'console_window'
  requestId: number
}

/** 必须与目标台词同处前端时间轴的语音变速/变调。 */
export interface ScriptVoiceShiftEvent extends ScriptEvent {
  type: 'voice_shift'
  rate?: number
  pitch?: number
}

/** 强制选择事件：鼠标被拖向 forced 选项，最终只能提交它 */
export interface ScriptForceChoiceEvent extends ScriptEvent {
  type: 'force_choice'
  choices: ScriptChoiceItem[]
  forced: string
}

export interface ScriptPoemWord {
  text: string
  warmPoints: number
  scriptPoints: number
  voidPoints: number
  glitch: boolean
}

/** 20 轮选词写诗互动；每轮包含 10 个词。 */
export interface ScriptPoemGameEvent extends ScriptEvent {
  type: 'poem_game'
  backgroundPath: string
  musicPath: string
  glitchMusicPath: string
  warmStickerPath: string
  scriptStickerPath: string
  voidStickerPath: string
  mode: 'normal' | 'act2' | 'act2_final'
  rounds: ScriptPoemWord[][]
  normalLoopStart: number
  glitchLoopStart: number
}

export interface ScriptErrorEvent extends ScriptEvent {
  type: 'error'
  error_code?: string
  message?: string
}

export interface ScriptStatusResetEvent extends ScriptEvent {
  type: 'status_reset'
  status?: string
}

export type ScriptEventType =
  | ScriptNarrationEvent
  | ScriptDialogueEvent
  | ScriptBackgroundEvent
  | ScriptPlayerEvent
  | ScriptModifyCharacterEvent
  | ScriptBackgroundEffectEvent
  | ScriptMusicEvent
  | ScriptSoundEvent
  | ScriptAmbientEvent
  | ScriptInputEvent
  | ScriptErrorEvent
  | ScriptStatusResetEvent
  | ScriptThinkingEvent
  | ScriptChapterChangeEvent
  | ScriptEndEvent
  | ScriptChoiceEvent
  | ScriptPresentPicEvent
  | ScriptFreeDialogueEvent
  | ScriptJumpscareEvent
  | ScriptWaitEvent
  | ScriptGlitchWindowEvent
  | ScriptConsoleWindowEvent
  | ScriptVoiceShiftEvent
  | ScriptForceChoiceEvent
  | ScriptPoemGameEvent

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
}

export interface ScriptSoundEvent extends ScriptEvent {
  type: 'sound'
  soundPath: string
}

export interface ScriptMusicEvent extends ScriptEvent {
  type: 'music'
  musicPath: string
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
}

export interface ScriptInputEvent extends ScriptEvent {
  type: 'input'
  hint: string
}
export interface ScriptChoiceEvent extends ScriptEvent {
  type: 'choice'
  choices: string[]
  allowFree: boolean
}
export interface ScriptEndEvent extends ScriptEvent {
  type: 'script_end'
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

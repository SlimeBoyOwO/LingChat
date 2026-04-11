export interface ScriptEvent {
  type: string;
  duration: number;
  isFinal?: boolean;
}

export interface ScriptChapterChangeEvent extends ScriptEvent {
  type: "chapter_change";
  chapterName: string;
}

export interface ScriptNarrationEvent extends ScriptEvent {
  type: "narration";
  text: string;
  displayName?: string;
  sceneId?: string;
}

export interface ScriptPlayerEvent extends ScriptEvent {
  type: "player";
  text: string;
  displayName?: string;
  displaySubtitle?: string;
  emotion?: string;
}

export interface ScriptDialogueEvent extends ScriptEvent {
  type: "reply";
  character?: string;
  roleId: number;
  emotion: string;
  originalTag: string;
  message: string;
  motionText: string;
  ttsText?: string;
  audioFile?: string;
  originalMessage: string;
  displayName?: string;
  displaySubtitle?: string;
}

export interface ScriptThinkingEvent extends ScriptEvent {
  type: "thinking";
  isThinking: boolean;
}

export interface ScriptBackgroundEvent extends ScriptEvent {
  type: "background";
  imagePath: string;
  transition: number;
}

export interface ScriptPresentPicEvent extends ScriptEvent {
  type: "present_pic";
  imagePath: string;
  scale: number;
}

export interface ScriptBackgroundEffectEvent extends ScriptEvent {
  type: "background_effect";
  effect: string;
}

export interface ScriptSoundEvent extends ScriptEvent {
  type: "background";
  soundPath: string;
}

export interface ScriptMusicEvent extends ScriptEvent {
  type: "music";
  musicPath: string;
}

export interface ScriptModifyCharacterEvent extends ScriptEvent {
  type: "modify_character";
  characterId: number;
  emotion?: string;
  action?: string;
}

export interface ScriptInputEvent extends ScriptEvent {
  type: "input";
  hint: string;
}
export interface ScriptChoiceEvent extends ScriptEvent {
  type: "input";
  choices: string[];
  allowFree: boolean;
}
export interface ScriptEndEvent extends ScriptEvent {
  type: "script_end";
}

export interface ScriptErrorEvent extends ScriptEvent {
  type: "error";
  error_code?: string;
  message?: string;
}

export interface ScriptStatusResetEvent extends ScriptEvent {
  type: "status_reset";
  status?: string;
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
  | ScriptInputEvent
  | ScriptErrorEvent
  | ScriptStatusResetEvent
  | ScriptThinkingEvent
  | ScriptChapterChangeEvent
  | ScriptEndEvent
  | ScriptChoiceEvent
  | ScriptPresentPicEvent;

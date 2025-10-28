export interface ScriptEvent {
  type: string;
  duration?: number;
  isFinal?: boolean;
}

export interface ScriptNarrationEvent extends ScriptEvent {
  type: "narration";
  text: string;
  sceneId?: string;
}

export interface ScriptPlayerEvent extends ScriptEvent {
  type: "player";
  text: string;
  emotion?: string;
}

export interface ScriptDialogueEvent extends ScriptEvent {
  type: "reply";
  character: string;
  emotion: string;
  originalTag: string;
  message: string;
  motionText: string;
  audioFile: string;
  originalMessage: string;
}

export interface ScriptBackgroundEvent extends ScriptEvent {
  type: "background";
  imagePath: string;
  transition?: string;
}

export type ScriptEventType =
  | ScriptNarrationEvent
  | ScriptDialogueEvent
  | ScriptBackgroundEvent
  | ScriptPlayerEvent;

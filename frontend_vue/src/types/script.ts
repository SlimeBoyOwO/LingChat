export interface ScriptEvent {
  type: string;
  isFinal?: boolean;
}

export interface ScriptNarrationEvent extends ScriptEvent {
  type: "narration";
  text: string;
  duration?: number;
  sceneId?: string;
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
  duration?: number;
}

export type ScriptEventType =
  | ScriptNarrationEvent
  | ScriptDialogueEvent
  | ScriptBackgroundEvent;

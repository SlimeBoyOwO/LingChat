export type WebSocketHandler = (data: any) => void;

export interface WebSocketMessage {
  type: string;
  data: any;
}

export interface WebSocketChatMessage {
  type: string;
  emotion: string;
  originalTag: string;
  message: string;
  motionText: string;
  audioFile: string;
  originalMessage: string;
  isFinal: boolean;
}

export enum WebSocketMessageTypes {
  MESSAGE = "message",
  REPLY = "reply",
  STATUS_UPDATE = "status_update",
  ERROR = "error",
}

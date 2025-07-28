export type WebSocketHandler = (data: any) => void;

export interface WebSocketMessage {
  type: string;
  data: any;
}

export interface WebSocketChatMessage {
  type: string;
  message: string;
  content?: string;
  emotion?: string;
  audioFile?: string;
  isMultiPart?: boolean;
  partIndex?: number;
  totalParts?: number;
  motionText?: string;
  originalTag?: string;
  isFinal?: boolean;
}

export enum WebSocketMessageTypes {
  MESSAGE = "message",
  REPLY = "reply",
  STATUS_UPDATE = "status_update",
  ERROR = "error",
}

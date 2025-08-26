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
  MESSAGE = "message", // 用户发送的信息
  AIREPLY = "reply", // 来自AI的回复
  SYSTEM_NARRATION = "narration", // 旁白
  STATUS_UPDATE = "status_update", // 静态资源更新
  ERROR = "error",
}

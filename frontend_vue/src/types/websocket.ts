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
  // 正常模式的消息类型
  MESSAGE = "message", // 用户发送的信息
  STATUS_UPDATE = "status_update", // 静态资源更新
  ERROR = "error",

  // 剧本模式下的消息类型
  SCRIPT_NARRATION = "narration", // 旁白
  SCRIPT_PLAYER = "player", // 玩家
  SCRIPT_DIALOGUE = "reply", // 角色对话

  SCRIPT_BACKGROUND = "background", // 旁白
  SCRIPT_MODIFY_CHARACTER = "modify_character", // 旁白

  SCRIPT_INPUT = "input", // 玩家输入

  SCRIPT_BACKGROUND_EFFECT = "background_effect",
  SCRIPT_MUSIC = "music",
  SCRIPT_SOUND = "sound",
}

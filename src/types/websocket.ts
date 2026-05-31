export type WebSocketHandler = (data: any) => void

export interface WebSocketMessage {
  type: string
  data: any
}

export interface WebSocketChatMessage {
  type: string
  emotion: string
  originalTag: string
  message: string
  motionText: string
  audioFile: string
  originalMessage: string
  isFinal: boolean
}

export enum WebSocketMessageTypes {
  // 正常模式的消息类型
  MESSAGE = 'message', // 用户发送的信息
  STATUS_UPDATE = 'status_update', // 静态资源更新
  ERROR = 'error',
  CONNECTION = 'connection_established', // 连接建立成功

  // 剧本模式下的消息类型
  SCRIPT_NARRATION = 'narration', // 旁白
  SCRIPT_PLAYER = 'player', // 玩家
  SCRIPT_DIALOGUE = 'reply', // 角色对话
  SCRIPT_THINKING = 'thinking', // 角色思考
  SCRIPT_FREE_DIALOGUE = 'free_dialogue', // 自由对话

  SCRIPT_CHAPTER_CHANGE = 'chapter_change', // 章节切换

  SCRIPT_BACKGROUND = 'background', // 旁白
  SCRIPT_PRESENT_PIC = 'present_pic', // 显示图片
  SCRIPT_MODIFY_CHARACTER = 'modify_character', // 旁白

  SCRIPT_INPUT = 'input', // 玩家输入
  SCRIPT_CHOICE = 'choice', // 玩家选择分支
  SCRIPT_END = 'script_end', // 剧本结束

  SCRIPT_BACKGROUND_EFFECT = 'background_effect',
  SCRIPT_MUSIC = 'music',
  SCRIPT_SOUND = 'sound',

  // 场景切换
  SCENE_CHANGE = 'scene_change',
}

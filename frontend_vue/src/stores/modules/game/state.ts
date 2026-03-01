export interface GameMessage {
  type: 'message' | 'reply'
  displayName: string
  content: string
  emotion?: string
  audioFile?: string
  isFinal?: boolean
  motionText?: string
  originalTag?: string
  timestamp?: number
}

export interface ScriptInfo {
  scriptName: string
  currentChapterName: string
  isRunning: boolean
}

export interface GameRole {
  roleId: number
  roleName: string
  roleSubTitle: string
  thinkMessage: string
  emotion: string
  originalEmotion: string
  scale: number
  offsetY: number
  offsetX: number
  bubbleTop: number
  bubbleLeft: number
  show: boolean
  clothes: object
  clothesName: string
  bodyPart: object
}

export interface GameState {
  runningScript: ScriptInfo | null

  gameRoles: Record<number, GameRole>
  presentRoleIds: number[]
  mainRoleId: number
  currentInteractRoleId: number

  userName: string
  userSubtitle: string

  currentLine: string
  currentStatus: 'input' | 'thinking' | 'responding' | 'presenting'
  dialogHistory: GameMessage[]

  command: string | null
}

export const state: GameState = {
  runningScript: null,

  gameRoles: {},
  presentRoleIds: [],
  mainRoleId: -1,
  currentInteractRoleId: -1,

  userName: '',
  userSubtitle: '',

  currentLine: '',
  currentStatus: 'input',
  dialogHistory: [],

  command: null,
}

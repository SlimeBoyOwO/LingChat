import type { Live2dSettings } from '@/types/live2d'
import type { SceneInfo } from '@/api/services/scene' // 导入场景类型
import type { ScriptChoiceItem, ScriptPoemGameEvent } from '@/types/script'

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
  /** 玩家消息序号（1-indexed），用于回溯定位 */
  userMessageSeq?: number
  /** 该轮生成的思考链（仅每轮最后一条回复消息有值） */
  thinking?: string
  /** 该台词的第二语言（日语）译文，日文界面下显示 */
  ttsText?: string
  /** 台词关联的角色 ID（null = 无角色，如工具调用回填行；生成语音计数时跳过） */
  senderRoleId?: number | null
}

export interface FreeDialogueInfo {
  isFreeDialogue: boolean
  maxRounds: number
  endLine: string
  currentRound: number
}

export interface ScriptInfo {
  scriptName: string
  currentChapterName: string
  choices: ScriptChoiceItem[]
  isRunning: boolean
  freeDialogueInfo: FreeDialogueInfo
  /** 剧本内容警告标记（如 'horror'）：恐怖剧本运行时用于锁定桌宠入口等 */
  contentWarning?: string
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
  scaleP: number
  offsetXP: number
  offsetYP: number
  bubbleTop: number
  bubbleLeft: number
  show: boolean
  clothes: object
  clothesName: string
  bodyPart: object
  live2d?: Live2dSettings | null
  character_folder: string
}

export interface GameState {
  runningScript: ScriptInfo | null
  /** 强制选择演出（DDLC 式鼠标拖拽）；非 null 时 ForceChoice 组件接管选择 */
  forceChoice: { requestId: string; choices: ScriptChoiceItem[]; forced: string } | null
  /** 选词写诗全屏互动；非 null 时 PoemGame 接管输入。 */
  poemGame: ScriptPoemGameEvent | null

  gameRoles: Record<number, GameRole>
  presentRoleIds: number[]
  /** 进入剧本前的在场角色快照；剧本演出的 hide_character 会改写 presentRoleIds，
      退出剧本时据此恢复，否则自由对话立绘消失（与后端 onstage/present 快照配套） */
  preScriptRoleIds: number[] | null
  mainRoleId: number
  currentInteractRoleId: number | null

  userName: string
  userSubtitle: string

  currentLine: string
  currentStatus: 'input' | 'thinking' | 'responding' | 'presenting'
  /** 当前思考链累计字数（用于实时显示“已深度思考 N 字”） */
  thinkingLength: number
  dialogHistory: GameMessage[]
  currentScene: SceneInfo | null // 当前加载的场景
  command: string | null

  initialized: boolean
  latestScreenshot: string | null
  /** 正在进行的截图 Promise，供 save handler 等待 */
  screenshotPending: Promise<string | null> | null
}

export const state: GameState = {
  runningScript: null,
  forceChoice: null,
  poemGame: null,

  gameRoles: {},
  presentRoleIds: [],
  preScriptRoleIds: null,
  mainRoleId: -1,
  currentInteractRoleId: -1,

  userName: '',
  userSubtitle: '',

  currentLine: '',
  currentStatus: 'input',
  thinkingLength: 0,
  dialogHistory: [],
  currentScene: null,
  command: null,

  initialized: false,
  latestScreenshot: null,
  screenshotPending: null,
}

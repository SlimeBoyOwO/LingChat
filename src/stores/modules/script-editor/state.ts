/**
 * 剧本编辑器 store —— 状态定义（setup 风格：每个字段是 ref）。
 *
 * 与 game store 的 option 风格不同：script-editor 的 action 大量互调兄弟方法，
 * option 风格里拆出 actions 会丢 this 类型（见 index.ts 头注释），故整体转 setup。
 * State 接口保留为「值的形状」文档；运行期状态是 useEditorState 返回的一组 ref。
 */
import { ref } from 'vue'
import type {
  AssetFile,
  AssetIndex,
  ChapterContent,
  GlobalCharacter,
  PreviewReadiness,
  ScriptDetail,
  ScriptPackage,
  ScriptSchema,
  ValidationReport,
  AssetFileIndex,
} from '@/api/services/script-editor'
import type { ScriptEventData } from '@/api/services/script-editor'

/** 编辑器外观偏好（背景图 + 模糊/压暗），持久化于 script-editor-ui */
export interface EditorBg {
  /** 自定义背景绝对路径；'' 表示用内置默认背景（background2.png，加载失败回退渐变） */
  path: string
  /** 背景模糊半径（px） */
  blur: number
  /** 压暗遮罩不透明度（0~1） */
  dim: number
}

export const DEFAULT_EDITOR_BG: EditorBg = { path: '', blur: 12, dim: 0.3 }

/** 撤销栈里的一帧：某个章节某一时刻的完整事件列表 */
export interface UndoFrame {
  chapterId: string
  name?: string
  events: ScriptEventData[]
  /** 该帧对应的选中下标，撤销后光标回到原处 */
  selected: number
}

/** 状态的「值形状」（非响应式视图），仅作类型/文档用；运行期见 useEditorState */
interface State {
  schema: ScriptSchema | null
  scripts: ScriptPackage[]
  loading: boolean
  detail: ScriptDetail | null
  globalAssets: AssetIndex
  chapter: ChapterContent | null
  selectedEvent: number
  dirty: boolean
  saving: boolean
  lastSavedAt: number | null
  undoStack: UndoFrame[]
  redoStack: UndoFrame[]
  report: ValidationReport | null
  previewing: boolean
  /** 当前试玩的会话代号（后端 GameStatus.preview_generation 的快照），
   *  用于丢弃上一轮试玩迟到的 ai:reply 事件；null 表示不在试玩中 */
  previewGeneration: number | null
  readiness: PreviewReadiness | null
  globalCharacters: GlobalCharacter[]
  /** 全局成就列表（成就 id → 标题），供解锁条件/成就事件的成就下拉选项 */
  achievements: Record<string, string>
  assetFiles: { script: AssetFileIndex | null; global: AssetFileIndex | null }
  level: 'flow' | 'chapter'
  tab:
    | 'flow'
    | 'config'
    | 'characters'
    | 'assets'
    | 'validate'
    | 'agent-chat'
    | 'agent-settings'
    | 'appearance'
  foldCompounds: boolean
  editorBg: EditorBg
  /** 背景图版本号：path 变更时自增，用于 asset URL 缓存击穿（不持久化） */
  bgVersion: number
  /** 全局背景库文件列表（game_data/backgrounds，带绝对路径），供外观页选择 */
  globalBgFiles: AssetFile[]
  /** 事件属性栏是否展开（临时态，不持久化；Ctrl+Space / 边缘手柄切换） */
  propsExpanded: boolean
  /** 事件属性栏展开时的宽度（px，拖拽手柄调整后记忆，持久化） */
  propsWidth: number
}

/** 撤销栈深度上限 */
export const UNDO_LIMIT = 100
/** 自动保存防抖（ms） */
export const AUTOSAVE_DELAY = 800
/** 校验比保存重得多（要扫全部剧本），所以单独用更长的防抖 */
export const VALIDATE_DELAY = 2500

export const emptyAssets = (): AssetIndex => ({
  background: [],
  music: [],
  sound: [],
  ambient: [],
  pic: [],
})

/** 响应式状态工厂：每个字段一个 ref，由 index.ts 的 setup store 组合 */
export const useEditorState = () => {
  const schema = ref<ScriptSchema | null>(null)
  const scripts = ref<ScriptPackage[]>([])
  const loading = ref(false)

  const detail = ref<ScriptDetail | null>(null)
  const globalAssets = ref<AssetIndex>(emptyAssets())

  const chapter = ref<ChapterContent | null>(null)
  const selectedEvent = ref(0)

  const dirty = ref(false)
  const saving = ref(false)
  const lastSavedAt = ref<number | null>(null)
  const undoStack = ref<UndoFrame[]>([])
  const redoStack = ref<UndoFrame[]>([])

  const report = ref<ValidationReport | null>(null)

  const previewing = ref(false)
  const previewGeneration = ref<number | null>(null)
  const readiness = ref<PreviewReadiness | null>(null)
  const globalCharacters = ref<GlobalCharacter[]>([])
  const achievements = ref<Record<string, string>>({})
  const assetFiles = ref<{ script: AssetFileIndex | null; global: AssetFileIndex | null }>({
    script: null,
    global: null,
  })

  const level = ref<State['level']>('flow')
  const tab = ref<State['tab']>('flow')
  const foldCompounds = ref(true)
  const editorBg = ref<EditorBg>({ ...DEFAULT_EDITOR_BG })
  const bgVersion = ref(0)
  const globalBgFiles = ref<AssetFile[]>([])
  const propsExpanded = ref(false)
  const propsWidth = ref(720)

  return {
    schema,
    scripts,
    loading,
    detail,
    globalAssets,
    chapter,
    selectedEvent,
    dirty,
    saving,
    lastSavedAt,
    undoStack,
    redoStack,
    report,
    previewing,
    previewGeneration,
    readiness,
    globalCharacters,
    achievements,
    assetFiles,
    level,
    tab,
    foldCompounds,
    editorBg,
    bgVersion,
    globalBgFiles,
    propsExpanded,
    propsWidth,
  }
}

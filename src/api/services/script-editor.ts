import { invoke } from '@tauri-apps/api/core'

/**
 * 剧本编辑器的后端接口。
 *
 * 这一层只做 invoke 封装，不含任何业务逻辑 —— 风格对齐 src/api/services/scene.ts。
 * 所有 YAML 语义都在 Rust 一侧，前端只操作 JSON。
 */

// ============================================================
// schema（由 Rust 导出，驱动全部表单与校验）
// ============================================================

export type FieldKind =
  | 'text'
  | 'textarea'
  | 'number'
  | 'bool'
  | 'select'
  | 'character'
  | 'emotion'
  | 'chapter'
  | 'asset'
  | 'choice_options'
  | 'branch_options'
  | 'var_options'
  | 'deprecated'

export type AssetKind = 'background' | 'music' | 'sound' | 'ambient' | 'pic'

export interface FieldSpec {
  key: string
  label: string
  kind: FieldKind
  required: boolean
  assetKind?: AssetKind
  options?: string[]
  placeholder?: string
  hint?: string
  enabled: boolean
}

export interface EventSpec {
  typeKey: string
  label: string
  category: string
  color: string
  fields: FieldSpec[]
}

export interface ActionSpec {
  typeKey: string
  label: string
  hint: string
  allowedIn: string[]
}

export interface UnlockConditionSpec {
  typeKey: string
  label: string
  fields: FieldSpec[]
}

export interface ConditionSyntax {
  supported: string[]
  unsupported: string[]
  note: string
}

export interface ScriptSchema {
  events: EventSpec[]
  commonFields: FieldSpec[]
  storyConfigFields: FieldSpec[]
  actionTypes: ActionSpec[]
  unlockConditionTypes: UnlockConditionSpec[]
  placeholderFields: string[]
  conditionSyntax: ConditionSyntax
}

// ============================================================
// 剧本包
// ============================================================

export type ScriptLayout = 'character' | 'standalone' | 'flat'

export interface ScriptPackage {
  key: string
  layout: ScriptLayout
  folderName: string
  boundCharacterFolder?: string
  scriptName: string
  description: string
  isAdventure: boolean
  chapterCount: number
  /** false 表示磁盘上有但引擎还没加载，需要 rescan 才能试玩 */
  loadedByEngine: boolean
}

export interface ChapterSummary {
  id: string
  name?: string
  /** 子目录，用于流程图分组 */
  group?: string
  eventCount: number
}

export interface AssetIndex {
  background: string[]
  music: string[]
  sound: string[]
  ambient: string[]
  pic: string[]
}

export interface ScriptCharacter {
  folder: string
  /** 剧本里 character: 应该写的值 */
  roleKey: string
  aiName: string
  emotions: string[]
  clothes: string[]
}

export interface ScriptDetail {
  package: ScriptPackage
  storyConfig: Record<string, unknown>
  chapters: ChapterSummary[]
  assets: AssetIndex
  characters: ScriptCharacter[]
}

/** 一个事件就是一个自由形状的 JSON 对象，字段由 schema 决定 */
export type ScriptEventData = Record<string, unknown>

export interface ChapterContent {
  id: string
  name?: string
  events: ScriptEventData[]
  extra: Record<string, unknown>
}

// ============================================================
// 校验
// ============================================================

export type Severity = 'error' | 'warn' | 'info'

export interface Diagnostic {
  severity: Severity
  /** 稳定的机器码，可用于过滤与跳转 */
  code: string
  message: string
  chapter?: string
  eventIndex?: number
  field?: string
}

export interface ValidationReport {
  diagnostics: Diagnostic[]
  errorCount: number
  warnCount: number
  infoCount: number
  /** 剧本里出现过的全部变量名 */
  variables: string[]
  /** 章节跳转边，从每章最后一条 chapter_end 反推 */
  edges: ChapterEdge[]
}

/** 一条章节跳转。to === 'end' 表示剧本结束 */
export interface ChapterEdge {
  from: string
  to: string
  isEnd: boolean
  /** 分支条件 / AI 分支名；linear 时不存在 */
  label?: string
  /** 该边所属章节的 end_type，流程图据此标注「条件分支 / AI 判定分支」 */
  endType: string
}

// ============================================================
// 命令
// ============================================================

export const getSchema = () => invoke<ScriptSchema>('editor_get_schema')

export const listScripts = () => invoke<ScriptPackage[]>('editor_list_scripts')

export const readScript = (key: string) => invoke<ScriptDetail>('editor_read_script', { key })

export const readChapter = (key: string, chapterId: string) =>
  invoke<ChapterContent>('editor_read_chapter', { key, chapterId })

export const validateScript = (key: string) =>
  invoke<ValidationReport>('editor_validate_script', { key })

export const writeChapter = (req: {
  key: string
  chapterId: string
  name?: string
  events: ScriptEventData[]
  extra?: Record<string, unknown>
}) => invoke<void>('editor_write_chapter', { req })

export const writeStoryConfig = (key: string, config: Record<string, unknown>) =>
  invoke<void>('editor_write_story_config', { key, config })

export const createChapter = (key: string, chapterId: string, name: string) =>
  invoke<ChapterContent>('editor_create_chapter', { key, chapterId, name })

export const deleteChapter = (key: string, chapterId: string) =>
  invoke<void>('editor_delete_chapter', { key, chapterId })

export const createScript = (req: {
  folderName: string
  scriptName?: string
  description?: string
  introChapter?: string
  isAdventure?: boolean
  boundCharacterFolder?: string
}) => invoke<ScriptPackage>('editor_create_script', { req })

export const deleteScript = (key: string) => invoke<void>('editor_delete_script', { key })

/**
 * 素材落点。
 * - script：只有这个剧本用，随剧本一起分发
 * - global：所有剧本共享，但导出剧本时不会带走
 *
 * 引擎的查找顺序是「先本剧本 Assets/，再全局 game_data/」，两种都能被找到。
 */
export type AssetScope = 'script' | 'global'

/**
 * 导入素材。只传源文件路径，由 Rust 自己复制 —— 与 import_font / importRoleFromPath
 * 的既有做法一致。不用 plugin-fs 读字节，因为用户从任意位置选的文件不在
 * capabilities 的 fs:scope 内，而且大文件转成数字数组走 IPC 会 OOM。
 */
export const uploadAsset = (key: string, kind: AssetKind, scope: AssetScope, srcPath: string) =>
  invoke<string>('editor_upload_asset', { key, kind, scope, srcPath })

/** 全局素材（game_data/backgrounds、musics、ambient） */
export const listGlobalAssets = () => invoke<AssetIndex>('editor_list_global_assets')

/** 全局角色库里的一个角色 */
export interface GlobalCharacter {
  folder: string
  aiName: string
  /** 在当前剧本里是否已导入 */
  alreadyInScript: boolean
  hasAvatar: boolean
}

export const listGlobalCharacters = (key: string) =>
  invoke<GlobalCharacter[]>('editor_list_global_characters', { key })

/**
 * 把全局角色导入剧本。复制的是 settings.yml —— 引擎解析 `character:` 只认
 * 剧本自己的 characters/，全局角色库不在那条查找路径上，所以必须复制而不是引用。
 *
 * 立绘默认不复制：get_avatar_file 本来就先找全局的 characters/<目录>/avatar，
 * 同名目录会自动命中。只有要把剧本单独分发出去时才需要 withAvatar。
 */
export const importGlobalCharacter = (key: string, folder: string, withAvatar: boolean) =>
  invoke<ScriptCharacter>('editor_import_global_character', { key, folder, withAvatar })

/** 试玩可行性：MAIN 会解析成谁，解析不到的话为什么 */
export interface PreviewReadiness {
  ok: boolean
  mainRoleName?: string
  boundCharacterFolder: string
  reason?: string
}

export const previewReadiness = (key: string) =>
  invoke<PreviewReadiness>('editor_preview_readiness', { key })

/**
 * 在编辑器里直接试玩。内部会先 rescan，fromChapter 留空则从开场章节开始。
 *
 * useLlm=false 时 AI 事件出固定占位文本，不花 token —— 调流程时不需要真生成。
 */
export const startPreview = (key: string, fromChapter: string | undefined, useLlm: boolean) =>
  invoke<void>('editor_start_preview', { key, fromChapter, useLlm })

/** 中止试玩 */
export const stopPreview = () => invoke<void>('editor_stop_preview')

export const createCharacter = (
  key: string,
  folder: string,
  aiName: string,
  systemPrompt: string,
) => invoke<ScriptCharacter>('editor_create_character', { key, folder, aiName, systemPrompt })

export const rescanScripts = () => invoke<number>('editor_rescan_scripts')

export const openScriptFolder = (key: string) => invoke<void>('editor_open_script_folder', { key })

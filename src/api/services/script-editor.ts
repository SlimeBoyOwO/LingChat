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

export const renameChapter = (key: string, from: string, to: string) =>
  invoke<void>('editor_rename_chapter', { key, from, to })

export const createScript = (req: {
  folderName: string
  scriptName?: string
  description?: string
  introChapter?: string
  isAdventure?: boolean
  boundCharacterFolder?: string
}) => invoke<ScriptPackage>('editor_create_script', { req })

export const deleteScript = (key: string) => invoke<void>('editor_delete_script', { key })

export const uploadAsset = (key: string, kind: AssetKind, fileName: string, data: Uint8Array) =>
  invoke<string>('editor_upload_asset', { key, kind, fileName, data: Array.from(data) })

export const createCharacter = (
  key: string,
  folder: string,
  aiName: string,
  systemPrompt: string,
) => invoke<ScriptCharacter>('editor_create_character', { key, folder, aiName, systemPrompt })

export const rescanScripts = () => invoke<number>('editor_rescan_scripts')

export const openScriptFolder = (key: string) => invoke<void>('editor_open_script_folder', { key })

/** 角色与插件压缩包共用的类型与约定。 */

export type ArchiveFormat = 'zip' | '7z'

/** 角色导入的冲突策略（角色目录名可自由取，故支持自动改名）。 */
export type ConflictPolicy = 'rename' | 'skip' | 'overwrite'

/**
 * 插件导入的冲突策略。
 * 插件目录名必须等于 `manifest.id`，改名会让后端拒绝加载，因此只有「覆盖」与「放弃」。
 */
export type PluginConflictPolicy = 'overwrite' | 'abort'

/** 导入结果的公共字段（角色与插件各自扩展）。 */
export interface ArchiveImportResult {
  conflict_action: string
  warnings: string[]
  bytes_extracted: number
  format: ArchiveFormat
}

/** 后端在生成 task_id 后立刻 emit，前端用来绑定取消按钮。 */
export interface ImportStartedEvent {
  task_id: string
}

export interface EntryEvent {
  phase: 'started' | 'entry' | 'finished' | 'error'
  index: number
  total: number
  name: string
  bytes_done: number
  bytes_total: number
  bytes_entry: number
}

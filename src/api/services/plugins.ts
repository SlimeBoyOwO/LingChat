import { invoke } from '@tauri-apps/api/core'
import type {
  ArchiveFormat,
  ArchiveImportResult,
  PluginConflictPolicy,
} from './archive'

export type { ArchiveFormat, PluginConflictPolicy } from './archive'

export type ConfigKind = 'string' | 'secret' | 'number' | 'boolean'

export interface ConfigFieldDecl {
  key: string
  label: string
  kind: ConfigKind
  required: boolean
  default?: unknown
}

export interface EnvDecl {
  key: string
  label: string
}

export interface PluginInfo {
  id: string
  name: string
  description: string
  version: string
  author?: string | null
  enabled: boolean
  config_schema: ConfigFieldDecl[]
  env: EnvDecl[]
  tools: string[]
  /** 插件声明携带的资源类型（characters / scripts / musics / backgrounds / ambients）。 */
  resources: string[]
  error?: string | null
}

/** 后端 plugins/resources.rs PluginResourceEntry 的镜像。 */
export type ResourceKind = 'characters' | 'scripts' | 'musics' | 'backgrounds' | 'ambients'

export interface PluginResourceEntry {
  kind: ResourceKind
  key: string
  name: string
  path: string
  plugin_id: string
  /** 与游戏现有资源同名冲突（默认使用游戏版）。 */
  conflict: boolean
  /** 已被软删除隐藏。 */
  hidden: boolean
}

export async function listPlugins(): Promise<PluginInfo[]> {
  return invoke('plugin_list')
}

export async function setPluginEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke('plugin_set_enabled', { id, enabled })
}

export async function savePluginConfig(
  id: string,
  config: Record<string, unknown>,
): Promise<void> {
  return invoke('plugin_save_config', { id, config })
}

export async function reloadPlugins(): Promise<void> {
  return invoke('plugin_reload')
}

export async function deletePlugin(id: string): Promise<void> {
  return invoke('plugin_delete', { id })
}

/** 列出某插件携带的全部资源（含 conflict / hidden 状态）。 */
export async function pluginResources(pluginId: string): Promise<PluginResourceEntry[]> {
  return invoke('plugin_resources', { pluginId })
}

/** 软删除：隐藏某插件资源（`key` 为 `<kind>/<key>` 标记）。 */
export async function pluginResourceHide(pluginId: string, key: string): Promise<void> {
  return invoke('plugin_resource_hide', { pluginId, key })
}

/** 恢复被隐藏的插件资源。 */
export async function pluginResourceRestore(pluginId: string, key: string): Promise<void> {
  return invoke('plugin_resource_restore', { pluginId, key })
}

/** 保留：把插件资源复制到游戏目录，成功后自动隐藏插件版。 */
export async function pluginResourceKeep(pluginId: string, key: string): Promise<void> {
  return invoke('plugin_resource_keep', { pluginId, key })
}

// ========== 插件压缩包导入 ==========

/** 后端 `plugins::importer::PluginImportResult` 的镜像。 */
export interface PluginImportResult extends ArchiveImportResult {
  plugin_id: string
  plugin_name: string
  version: string
  /** 该插件声明携带的资源类型。 */
  resources: string[]
}

/**
 * 从桌面文件路径或 Android SAF 内容 URI 导入插件压缩包。
 * 目标目录名取 manifest.id，冲突只允许「覆盖」或「放弃」。
 */
export async function importPluginFromPath(params: {
  path: string
  format?: ArchiveFormat // 可选；后端用 magic 裁决
  conflict: PluginConflictPolicy
}): Promise<PluginImportResult> {
  return invoke<PluginImportResult>('import_plugin_from_path', {
    path: params.path,
    format: params.format ?? null,
    conflict: params.conflict,
  })
}

/** 取消进行中的插件导入（task_id 来自 `plugin:import-started` 事件）。 */
export async function cancelPluginImport(taskId: string): Promise<void> {
  await invoke('cancel_plugin_import', { taskId })
}

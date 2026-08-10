import { invoke } from '@tauri-apps/api/core'

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
  error?: string | null
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

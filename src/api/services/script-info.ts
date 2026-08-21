import { invoke } from '@tauri-apps/api/core'

export interface CharacterSettings {
  ai_name: string
  ai_subtitle: string
  thinking_message: string
  scale: number
  offset_x: number
  offset_y: number
  bubble_top: number
  bubble_left: number
  clothes: object
  clothes_name: string
  body_part: object
}

export interface ScriptSummary {
  script_name: string
  description?: string
  folder_key?: string
  intro_chapter?: string
  content_warning?: string
}

export interface ScriptInfo {
  script_name: string
  characters: {
    [character_id: string]: CharacterSettings
  }
}

export const getScriptList = async (): Promise<ScriptSummary[]> => {
  try {
    const data = await invoke<{ scripts: ScriptSummary[] }>('list_scripts')
    return data.scripts
  } catch (error: any) {
    console.error('获取剧本列表错误:', error)
    throw error
  }
}

export const getStandaloneScriptList = async (): Promise<ScriptSummary[]> => {
  try {
    const data = await invoke<{ scripts: ScriptSummary[] }>('list_standalone_scripts')
    return data.scripts
  } catch (error: any) {
    console.error('获取独立剧本列表错误:', error)
    throw error
  }
}

export const getScriptInfo = async (scriptName: string): Promise<ScriptInfo> => {
  // Script info is initialized when the script starts via start_script command
  try {
    const data = await invoke<ScriptInfo>('get_script_info', { scriptName })
    console.log('Script信息:', data)
    return data
  } catch (error: any) {
    console.error('获取脚本信息错误:', error)
    throw error
  }
}

export const startScript = async (scriptName: string): Promise<void> => {
  try {
    await invoke('start_script', { scriptName })
  } catch (error: any) {
    console.error('启动剧本错误:', error)
    throw error
  }
}

// 清除剧本的持久化运行状态（周目记忆），下次进入从第一周目重新开始。
// 返回 true 表示确实有记忆被清掉。
export const resetScriptState = async (scriptName: string): Promise<boolean> => {
  try {
    return await invoke<boolean>('reset_script_state', { scriptName })
  } catch (error: any) {
    console.error('重置剧本记忆错误:', error)
    throw error
  }
}

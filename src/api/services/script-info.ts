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
  // Standalone scripts are included in the full list; filter client-side
  try {
    const scripts = await getScriptList()
    return scripts // Backend returns all scripts; standalone filter is done by caller if needed
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

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
  /** 剧本声明了 persistent_vars（跨局记忆）时为 true，前端据此显示「重置记忆」 */
  has_persistent_vars?: boolean
  /** 来源："game" 或提供该剧本的插件 id。 */
  source?: string
  plugin_id?: string | null
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

export interface ScriptGhostLock {
  locked: boolean
  /** 锁定中时为该剧本 Assets 目录绝对路径（convertFileSrc 加载素材用） */
  asset_dir?: string
}

// 删角色文件彩蛋（DDLC ghost menu 对应物）：.chr 被全删的剧本进入时锁成幽灵演出。
// 进入前实时查询，避免列表缓存过期——玩家可能刚在另一个窗口删完/放回文件。
export const checkScriptGhostLock = async (scriptName: string): Promise<ScriptGhostLock> => {
  try {
    return await invoke<ScriptGhostLock>('check_script_ghost_lock', { scriptName })
  } catch (error: any) {
    console.error('检查剧本幽灵锁定错误:', error)
    // 判定失败宁可放行，不能把玩家正常剧本锁在门外
    return { locked: false }
  }
}

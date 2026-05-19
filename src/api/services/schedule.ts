import { invoke } from '@tauri-apps/api/core'

export interface ScheduleData {
  scheduleGroups?: Record<string, any>
  todoGroups?: Record<string, any>
  importantDays?: any[]
}

export const getSchedules = async (): Promise<ScheduleData> => {
  try {
    const data = await invoke<ScheduleData>('get_schedules')
    return data
  } catch (error: any) {
    console.error('获取日程信息错误:', error.message)
    throw error
  }
}

export const saveSchedules = async (data: ScheduleData): Promise<void> => {
  try {
    await invoke('save_schedules', { data })
  } catch (error: any) {
    console.error('保存日程信息错误:', error.message)
    throw error
  }
}

export const reloadProactiveSystem = async (): Promise<void> => {
  try {
    await invoke('reload_proactive_system')
  } catch (error: any) {
    console.error('重载主动系统错误:', error.message)
    throw error
  }
}

import http from '../http'

export interface ScheduleData {
  scheduleGroups?: Record<string, any>
  todoGroups?: Record<string, any>
  importantDays?: any[]
}

export const getSchedules = async (): Promise<ScheduleData> => {
  try {
    const data = await http.get('/v1/chat/schedule/get_schedules')
    return data
  } catch (error: any) {
    console.error('获取日程信息错误:', error.message)
    throw error
  }
}

export const saveSchedules = async (data: ScheduleData): Promise<void> => {
  try {
    await http.post('/v1/chat/schedule/save_schedules', data)
  } catch (error: any) {
    console.error('保存日程信息错误:', error.message)
    throw error
  }
}

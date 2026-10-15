import { invoke } from "@tauri-apps/api/core";

export interface ScheduleData {
  scheduleGroups?: Record<string, any>;
  todoGroups?: Record<string, any>;
  importantDays?: any[];
}

export const getSchedules = async (): Promise<ScheduleData> => {
  try {
    const data = await invoke<ScheduleData>("get_schedules");
    return data;
  } catch (error: any) {
    console.error("获取日程信息错误:", error.message);
    throw error;
  }
};

export const saveSchedules = async (data: ScheduleData): Promise<void> => {
  try {
    console.log("日程信息触发提醒");
    await invoke("save_schedules", { data });
  } catch (error: any) {
    console.error("保存日程信息错误:", error.message);
    throw error;
  }
};

/** 把「期望间隔（分钟）」换算成后端实际使用的兴趣增长随机范围 [下限, 上限]。 */
export const previewProactiveInterestRange = async (minutes: number): Promise<[number, number]> => {
  return invoke<[number, number]>("preview_proactive_interest_range", { minutes });
};

export const reloadProactiveSystem = async (): Promise<void> => {
  try {
    await invoke("reload_proactive_system");
  } catch (error: any) {
    console.error("重载主动系统错误:", error.message);
    throw error;
  }
};

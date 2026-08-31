import { invoke } from '@tauri-apps/api/core'
import type { Achievement } from '@/stores/modules/ui/achievement'

export const getAchievementList = async (): Promise<Record<string, Achievement>> => {
  return invoke('get_achievement_list')
}

export const unlockAchievement = async (achievementId: string): Promise<void> => {
  await invoke('unlock_achievement', { achievementId })
}

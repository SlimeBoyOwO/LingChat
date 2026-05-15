import http from '@/api/http'
import type { Achievement } from '@/stores/modules/ui/achievement'

export const getAchievementList = async (): Promise<Record<string, Achievement>> => {
  return http.get('/v1/chat/achievement/list')
}

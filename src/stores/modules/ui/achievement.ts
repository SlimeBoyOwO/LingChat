import { defineStore } from 'pinia'
import { getAchievementList } from '@/api/services/achievement'
import { registerHandler, sendWebSocketMessage } from '@/api/websocket'

export type AchievementType = 'common' | 'rare'

export interface Achievement {
  id: string
  title: string
  description: string
  type: AchievementType
  imgUrl?: string
  audioUrl?: string
  duration?: number
  unlocked?: boolean
  unlocked_at?: string
  current_progress?: number
  target_progress?: number
}

interface AchievementState {
  queue: Achievement[]
  current: Achievement | null
  isVisible: boolean
  allAchievements: Record<string, Achievement>
}

const DEFAULT_DURATION = 3500

export const useAchievementStore = defineStore('achievement', {
  state: (): AchievementState => ({
    queue: [],
    current: null,
    isVisible: false,
    allAchievements: {},
  }),

  actions: {
    addAchievement(achievement: Omit<Achievement, 'id'>) {
      const id = Date.now().toString() + Math.random().toString(36).substring(2)
      this.queue.push({
        id,
        duration: DEFAULT_DURATION,
        ...achievement,
      } as Achievement)
      this.processQueue()
    },

    processQueue() {
      if (this.isVisible || this.queue.length === 0) return

      const next = this.queue.shift()
      if (next) {
        this.current = next

        this.current.duration = this.current.duration || DEFAULT_DURATION
        this.current.audioUrl =
          this.current.audioUrl ||
          (this.current.type === 'common'
            ? '/audio_effects/achievement_common.wav'
            : '/audio_effects/achievement_rare.wav')
        this.isVisible = true

        setTimeout(() => {
          this.hideAchievement()
        }, this.current.duration)
      }
    },

    hideAchievement() {
      this.isVisible = false

      setTimeout(() => {
        this.current = null
        this.processQueue()
      }, 500)
    },

    /**
     * 通知后端请求解锁成就
     */
    notifyBackendUnlock(achievementData: Omit<Achievement, 'id'>) {
      sendWebSocketMessage('achievement.unlock_request', achievementData)
    },

    /**
     * 获取所有成就列表
     */
    async fetchAchievements() {
      try {
        const data = await getAchievementList()
        if (data) {
          this.allAchievements = data
        }
      } catch (error) {
        console.error('获取成就列表失败:', error)
      }
    },

    /**
     * 监听后端推送的成就解锁消息
     */
    listenForUnlocks() {
      // 监听解锁通知
      registerHandler('achievement.unlocked', (message) => {
        if (message.data) {
          const { id, title, description, type, imgUrl, audioUrl, duration } = message.data

          // 更新列表中的状态
          if (id && this.allAchievements[id]) {
            this.allAchievements[id].unlocked = true
            this.allAchievements[id].unlocked_at = new Date().toISOString()
            this.allAchievements[id].current_progress = this.allAchievements[id].target_progress
          }

          this.queue.push({
            id,
            title,
            description,
            type,
            imgUrl,
            audioUrl,
            duration: duration || DEFAULT_DURATION,
          })
          this.processQueue()
        }
      })
    },
  },
})

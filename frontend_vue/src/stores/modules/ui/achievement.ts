import { defineStore } from 'pinia'
import { registerHandler, sendWebSocketMessage } from '@/api/websocket'

export type AchievementType = 'common' | 'rare'

export interface Achievement {
  id: string
  title: string
  message: string
  type: AchievementType
  imgUrl?: string
  audioUrl?: string
  duration?: number
}

interface AchievementState {
  queue: Achievement[]
  current: Achievement | null
  isVisible: boolean
}

const DEFAULT_DURATION = 3500

export const useAchievementStore = defineStore('achievement', {
  state: (): AchievementState => ({
    queue: [],
    current: null,
    isVisible: false,
  }),

  actions: {
    addAchievement(achievement: Omit<Achievement, 'id'>) {
      const id = Date.now().toString() + Math.random().toString(36).substring(2)
      this.queue.push({
        id,
        duration: DEFAULT_DURATION,
        ...achievement,
      })
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
     * 监听后端推送的成就解锁消息
     */
    listenForUnlocks() {
      registerHandler('achievement.unlocked', (message) => {
        if (message.data) {
          const { id, title, message: msg, type, imgUrl, audioUrl, duration } = message.data
          // 如果后端没传id，我们自己生成一个，但理想情况下应该用后端的
          if (!id) {
            this.addAchievement({
              title,
              message: msg,
              type,
              imgUrl,
              audioUrl,
              duration: duration || DEFAULT_DURATION,
            })
          } else {
            // 如果后端传了完整数据
            this.queue.push({
              id,
              title,
              message: msg,
              type,
              imgUrl,
              audioUrl,
              duration: duration || DEFAULT_DURATION,
            })
            this.processQueue()
          }
        }
      })
    },
  },
})

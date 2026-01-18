import { defineStore } from 'pinia'

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
        this.isVisible = true

        setTimeout(() => {
          this.hideAchievement()
        }, next.duration || DEFAULT_DURATION)
      }
    },

    hideAchievement() {
      this.isVisible = false

      setTimeout(() => {
        this.current = null
        this.processQueue()
      }, 500)
    },
  },
})

import { defineStore } from 'pinia'
import {
  getCharacterAdventures,
  getAllAdventures,
  startAdventure,
  checkUnlocks,
  resetAdventure,
  type AdventureInfo,
  type UnlockedAdventure,
} from '@/api/services/adventure'

export interface AdventureState {
  currentCharacterAdventures: AdventureInfo[]
  allAdventures: AdventureInfo[]
  unlockNotifications: UnlockedAdventure[]
  loading: boolean
}

export const useAdventureStore = defineStore('adventure', {
  state: (): AdventureState => ({
    currentCharacterAdventures: [],
    allAdventures: [],
    unlockNotifications: [],
    loading: false,
  }),

  getters: {
    unlockedCount: (state) => {
      return state.currentCharacterAdventures.filter((adv) => adv.status !== 'locked').length
    },

    completedCount: (state) => {
      return state.currentCharacterAdventures.filter((adv) => adv.status === 'completed').length
    },

    inProgressAdventures: (state) => {
      return state.currentCharacterAdventures.filter((adv) => adv.status === 'in_progress')
    },

    sortedAdventures: (state) => {
      return [...state.currentCharacterAdventures].sort((a, b) => a.order - b.order)
    },
  },

  actions: {
    async fetchCharacterAdventures(characterFolder: string) {
      this.loading = true
      try {
        this.currentCharacterAdventures = await getCharacterAdventures(characterFolder)
      } catch (error) {
        console.error('[AdventureStore] Failed to fetch adventures:', error)
        throw error
      } finally {
        this.loading = false
      }
    },

    async fetchAllAdventures() {
      this.loading = true
      try {
        this.allAdventures = await getAllAdventures()
      } catch (error) {
        console.error('获取所有冒险列表失败:', error)
        throw error
      } finally {
        this.loading = false
      }
    },

    async startAdventure(adventureFolder: string) {
      try {
        await startAdventure(adventureFolder)
        const adventure = this.currentCharacterAdventures.find(
          (adv) => adv.adventure_folder === adventureFolder,
        )
        if (adventure) {
          adventure.status = 'in_progress'
        }
      } catch (error) {
        console.error('启动冒险失败:', error)
        throw error
      }
    },

    async checkUnlocks() {
      try {
        const newlyUnlocked = await checkUnlocks()
        if (newlyUnlocked.length > 0) {
          this.unlockNotifications.push(...newlyUnlocked)
        }
        return newlyUnlocked
      } catch (error) {
        console.error('检测冒险解锁失败:', error)
        throw error
      }
    },

    async resetAdventure(adventureFolder: string) {
      try {
        await resetAdventure(adventureFolder)
        const adventure = this.currentCharacterAdventures.find(
          (adv) => adv.adventure_folder === adventureFolder,
        )
        if (adventure) {
          adventure.status = 'unlocked'
          adventure.completed_at = undefined
        }
      } catch (error) {
        console.error('重置冒险失败:', error)
        throw error
      }
    },

    popUnlockNotification(): UnlockedAdventure | undefined {
      return this.unlockNotifications.shift()
    },

    clearUnlockNotifications() {
      this.unlockNotifications = []
    },

    markAdventureCompleted(adventureFolder: string) {
      const adventure = this.currentCharacterAdventures.find(
        (adv) => adv.adventure_folder === adventureFolder,
      )
      if (adventure) {
        adventure.status = 'completed'
        adventure.completed_at = new Date().toISOString()
      }
    },
  },
})

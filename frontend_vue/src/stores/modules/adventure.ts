import { defineStore } from 'pinia'
import {
  getCharacterAdventures,
  getAllAdventures,
  getUserProgress,
  startAdventure,
  checkUnlocks,
  resetAdventure,
  type AdventureInfo,
  type AdventureProgress,
  type UnlockedAdventure,
} from '@/api/services/adventure'
import { useUserStore } from './user/user'

export interface AdventureState {
  // 当前角色的冒险列表
  currentCharacterAdventures: AdventureInfo[]
  // 所有冒险列表
  allAdventures: AdventureInfo[]
  // 用户的冒险进度
  userProgress: AdventureProgress[]
  // 当前用户ID（TODO: 多用户支持时从用户系统获取）
  currentUserId: number
  // 新解锁的冒险通知队列
  unlockNotifications: UnlockedAdventure[]
  // 是否正在加载
  loading: boolean
}

export const useAdventureStore = defineStore('adventure', {
  state: (): AdventureState => ({
    currentCharacterAdventures: [],
    allAdventures: [],
    userProgress: [],
    currentUserId: 1, // 默认用户ID
    unlockNotifications: [],
    loading: false,
  }),

  getters: {
    /**
     * 获取已解锁的冒险数量
     */
    unlockedCount: (state) => {
      return state.currentCharacterAdventures.filter((adv) => adv.status !== 'locked').length
    },

    /**
     * 获取已完成的冒险数量
     */
    completedCount: (state) => {
      return state.currentCharacterAdventures.filter((adv) => adv.status === 'completed').length
    },

    /**
     * 获取进行中的冒险
     */
    inProgressAdventures: (state) => {
      return state.currentCharacterAdventures.filter((adv) => adv.status === 'in_progress')
    },

    /**
     * 按order排序的冒险列表
     */
    sortedAdventures: (state) => {
      return [...state.currentCharacterAdventures].sort((a, b) => a.order - b.order)
    },
  },

  actions: {
    /**
     * 获取指定角色的冒险列表
     */
    async fetchCharacterAdventures(characterFolder: string) {
      this.loading = true
      console.log('[AdventureStore] Fetching adventures for:', characterFolder)
      try {
        this.currentCharacterAdventures = await getCharacterAdventures(
          characterFolder,
          this.currentUserId,
        )
        console.log('[AdventureStore] Fetched adventures:', this.currentCharacterAdventures)
      } catch (error) {
        console.error('[AdventureStore] Failed to fetch adventures:', error)
        throw error
      } finally {
        this.loading = false
      }
    },

    /**
     * 获取所有冒险列表
     */
    async fetchAllAdventures() {
      this.loading = true
      try {
        this.allAdventures = await getAllAdventures(this.currentUserId)
      } catch (error) {
        console.error('获取所有冒险列表失败:', error)
        throw error
      } finally {
        this.loading = false
      }
    },

    /**
     * 获取用户冒险进度
     */
    async fetchUserProgress() {
      try {
        this.userProgress = await getUserProgress(this.currentUserId)
      } catch (error) {
        console.error('获取用户冒险进度失败:', error)
        throw error
      }
    },

    /**
     * 启动冒险
     */
    async startAdventure(adventureFolder: string) {
      const userStore = useUserStore()
      try {
        await startAdventure(this.currentUserId, userStore.client_id, adventureFolder)
        // 更新本地状态
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

    /**
     * 检测新解锁的冒险
     */
    async checkUnlocks() {
      try {
        const newlyUnlocked = await checkUnlocks(this.currentUserId)
        if (newlyUnlocked.length > 0) {
          // 添加到通知队列
          this.unlockNotifications.push(...newlyUnlocked)
          // 刷新冒险列表
          // 注意：这里需要知道当前角色，可能需要从其他store获取
        }
        return newlyUnlocked
      } catch (error) {
        console.error('检测冒险解锁失败:', error)
        throw error
      }
    },

    /**
     * 重置冒险
     */
    async resetAdventure(adventureFolder: string) {
      try {
        await resetAdventure(this.currentUserId, adventureFolder)
        // 更新本地状态
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

    /**
     * 显示解锁通知（从队列中取出第一个）
     */
    popUnlockNotification(): UnlockedAdventure | undefined {
      return this.unlockNotifications.shift()
    },

    /**
     * 清空解锁通知队列
     */
    clearUnlockNotifications() {
      this.unlockNotifications = []
    },

    /**
     * 标记冒险为已完成（由WebSocket事件触发）
     */
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

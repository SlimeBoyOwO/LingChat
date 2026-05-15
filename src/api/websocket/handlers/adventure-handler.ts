import { registerHandler } from '..'
import { useAdventureStore } from '../../../stores/modules/adventure'

export class AdventureHandler {
  constructor() {
    this.registerHandlers()
  }

  private registerHandlers() {
    // 处理冒险解锁事件
    registerHandler('adventure_unlock', (message: any) => {
      console.log('收到冒险解锁事件:', message)
      const adventureStore = useAdventureStore()

      if (message.data) {
        // 将解锁的冒险添加到通知队列
        adventureStore.unlockNotifications.push(message.data)
      }
    })

    // 处理冒险完成事件（可选，如果后端会发送）
    registerHandler('adventure_complete', (message: any) => {
      console.log('收到冒险完成事件:', message)
      const adventureStore = useAdventureStore()

      if (message.data?.adventure_folder) {
        adventureStore.markAdventureCompleted(message.data.adventure_folder)
      }
    })
  }
}

// 导出单例实例
export const adventureHandler = new AdventureHandler()

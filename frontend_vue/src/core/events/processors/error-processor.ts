import type { IEventProcessor } from '../event-processor'
import type { ScriptErrorEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class ErrorProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'error'
  }

  async processEvent(event: ScriptErrorEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    console.log('处理错误事件:', event)

    // 使用 error_code 查询对应的角色专属提示
    uiStore.showError({
      errorCode: event.error_code || 'default_error',
    })

    // 重置游戏状态
    gameStore.currentStatus = 'input'
    gameStore.currentLine = ''
    console.log('游戏状态已重置为: input (由错误处理器触发)')
  }
}

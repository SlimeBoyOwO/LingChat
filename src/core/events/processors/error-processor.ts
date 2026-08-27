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

    // 使用 error_code 查询 i18n/角色提示；未知错误（other）附带原始错误便于排查
    const errorCode = event.error_code || 'default_error'
    uiStore.showError({
      errorCode,
      message: errorCode === 'other' ? event.message || undefined : undefined,
    })

    // 重置游戏状态
    gameStore.currentStatus = 'input'
    gameStore.currentLine = ''
    console.log('游戏状态已重置为: input (由错误处理器触发)')
  }
}

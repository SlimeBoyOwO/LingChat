import type { IEventProcessor } from '../event-processor'
import type { ScriptStatusResetEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'

export default class StatusResetProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'status_reset'
  }

  async processEvent(event: ScriptStatusResetEvent): Promise<void> {
    const gameStore = useGameStore()

    console.log('处理状态重置事件:', event)

    gameStore.currentStatus =
      (event.status as 'input' | 'thinking' | 'responding' | 'presenting') || 'input'
    gameStore.currentLine = ''
    console.log('游戏状态已重置为:', event.status || 'input')
  }
}

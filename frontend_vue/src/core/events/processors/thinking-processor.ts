import type { IEventProcessor } from '../event-processor'
import type { ScriptThinkingEvent } from '../../../types'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class ThinkingProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'thinking'
  }

  async processEvent(event: ScriptThinkingEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    // 更新游戏状态显示对话
    gameStore.currentStatus = 'thinking'
  }
}

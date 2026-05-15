import type { IEventProcessor } from '../event-processor'
import type { ScriptThinkingEvent } from '../../../types'
import { useGameStore } from '@/stores/modules/game'

export default class ThinkingProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'thinking'
  }

  async processEvent(event: ScriptThinkingEvent): Promise<void> {
    const gameStore = useGameStore()

    if (event.isThinking) {
      // AI 开始思考，锁定输入
      gameStore.currentStatus = 'thinking'
    } else {
      // AI 停止思考（通常是错误恢复），重置到可输入状态
      gameStore.currentStatus = 'input'
      gameStore.currentLine = ''
    }
  }
}

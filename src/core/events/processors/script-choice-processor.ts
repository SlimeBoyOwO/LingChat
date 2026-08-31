import type { IEventProcessor } from '../event-processor'
import type { ScriptChoiceEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { WebSocketMessageTypes } from '../../../types'

export default class ChoiceProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === WebSocketMessageTypes.SCRIPT_CHOICE
  }

  async processEvent(event: ScriptChoiceEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    // 更新游戏状态
    if (gameStore.runningScript) {
      gameStore.runningScript.choices = event.choices
    }

    gameStore.currentStatus = 'input'
  }
}

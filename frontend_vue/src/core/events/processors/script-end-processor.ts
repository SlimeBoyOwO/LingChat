import type { IEventProcessor } from '../event-processor'
import type { ScriptEndEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { WebSocketMessageTypes } from '../../../types'

export default class ScriptEndProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === WebSocketMessageTypes.SCRIPT_END
  }

  async processEvent(event: ScriptEndEvent): Promise<void> {
    const gameStore = useGameStore()
    gameStore.exitStoryMode()
    const uiStore = useUIStore()
    uiStore.showPlayerHintLine = ''
  }
}

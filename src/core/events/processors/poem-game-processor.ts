import type { IEventProcessor } from '../event-processor'
import type { ScriptPoemGameEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'

export default class PoemGameProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'poem_game'
  }

  async processEvent(event: ScriptPoemGameEvent): Promise<void> {
    const gameStore = useGameStore()
    gameStore.poemGame = event
    gameStore.currentStatus = 'input'
  }
}

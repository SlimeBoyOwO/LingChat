import type { IEventProcessor } from '../event-processor'
import type { ScriptForceChoiceEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'

export default class ForceChoiceProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'force_choice'
  }

  async processEvent(event: ScriptForceChoiceEvent): Promise<void> {
    const gameStore = useGameStore()

    gameStore.forceChoice = {
      requestId: event.requestId,
      choices: event.choices,
      forced: event.forced,
    }
    gameStore.currentStatus = 'input'
  }
}

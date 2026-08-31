import type { IEventProcessor } from '../event-processor'
import { WebSocketMessageTypes, type ScriptFreeDialogueEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'

export default class FreeDialogueProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === WebSocketMessageTypes.SCRIPT_FREE_DIALOGUE
  }

  async processEvent(event: ScriptFreeDialogueEvent): Promise<void> {
    const gameStore = useGameStore()

    // 处理对话逻辑
    if (!gameStore.runningScript) return

    const freeDialogue = gameStore.runningScript.freeDialogueInfo

    freeDialogue.isFreeDialogue = event.switch

    if (freeDialogue.isFreeDialogue) {
      freeDialogue.maxRounds = event.maxRounds
      freeDialogue.endLine = event.endLine
    } else {
      freeDialogue.currentRound = 0
      freeDialogue.maxRounds = 0
      freeDialogue.endLine = ''
    }
  }
}

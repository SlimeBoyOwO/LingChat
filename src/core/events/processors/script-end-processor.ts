import type { IEventProcessor } from '../event-processor'
import type { ScriptEndEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { WebSocketMessageTypes } from '../../../types'
import { useAdventureStore } from '@/stores/modules/adventure'

export default class ScriptEndProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === WebSocketMessageTypes.SCRIPT_END
  }

  async processEvent(event: ScriptEndEvent): Promise<void> {
    const adventureStore = useAdventureStore()
    if (adventureStore.inProgressAdventures.length > 0) {
      // 按道理来讲，应该只有一个进行的剧本哈，但是为了保险起见，还是遍历一下
      for (const adventure of adventureStore.inProgressAdventures) {
        adventureStore.markAdventureCompleted(adventure.adventure_folder)
      }
    }

    const gameStore = useGameStore()
    gameStore.exitStoryMode()
    const uiStore = useUIStore()
    uiStore.showPlayerHintLine = ''
  }
}

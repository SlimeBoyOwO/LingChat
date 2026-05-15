import type { IEventProcessor } from '../event-processor'
import type { ScriptChapterChangeEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { WebSocketMessageTypes } from '../../../types'

export default class ChapterChangeProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === WebSocketMessageTypes.SCRIPT_CHAPTER_CHANGE
  }

  async processEvent(event: ScriptChapterChangeEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()
    if (gameStore.runningScript) gameStore.runningScript.currentChapterName = event.chapterName
  }
}

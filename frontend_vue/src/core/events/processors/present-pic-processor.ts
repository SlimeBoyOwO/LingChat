import type { IEventProcessor } from '../event-processor'
import { WebSocketMessageTypes, type ScriptPresentPicEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class PresentPicProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === WebSocketMessageTypes.SCRIPT_PRESENT_PIC
  }

  async processEvent(event: ScriptPresentPicEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    // 处理对话逻辑
    gameStore.currentStatus = 'presenting'

    let url = event.imagePath
      ? `/api/v1/chat/script/pic_file/${encodeURIComponent(event.imagePath)}`
      : ''

    uiStore.currentPresentPic = url
    uiStore.currentPresentPicScale = event.scale
  }
}

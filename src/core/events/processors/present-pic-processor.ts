import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'
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

    gameStore.currentStatus = 'presenting'

    let url = ''

    if (event.imagePath) {
      try {
        const path = await invoke<string>('get_script_media_file', {
          filePath: event.imagePath,
          mediaType: 'pic',
        })
        url = convertFileSrc(path)
      } catch {
        url = ''
      }
    }

    uiStore.currentPresentPic = url
    uiStore.currentPresentPicScale = event.scale
  }
}

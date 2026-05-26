import { convertFileSrc } from '@tauri-apps/api/core'
import type { IEventProcessor } from '../event-processor'
import type { ScriptBackgroundEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class BackgroundProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'background'
  }

  async processEvent(event: ScriptBackgroundEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    gameStore.currentStatus = 'presenting'

    let url = '../pictures/background/default.png'

    if (event.imagePath) {
      try {
        url = convertFileSrc(event.imagePath)
      } catch {
        url = '../pictures/background/default.png'
      }
    }

    uiStore.currentBackgroundTransition = event.transition * 1000
    uiStore.setCurrentBackground(url)
  }
}

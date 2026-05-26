import { convertFileSrc } from '@tauri-apps/api/core'
import type { IEventProcessor } from '../event-processor'
import type { ScriptSoundEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class SoundProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'sound'
  }

  async processEvent(event: ScriptSoundEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    gameStore.currentStatus = 'presenting'

    let url = 'None'

    if (event.soundPath) {
      try {
        url = convertFileSrc(event.soundPath)
      } catch {
        url = 'None'
      }
    }

    uiStore.currentSoundEffect = url
  }
}

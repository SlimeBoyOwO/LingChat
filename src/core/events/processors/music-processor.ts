import { convertFileSrc } from '@tauri-apps/api/core'
import type { IEventProcessor } from '../event-processor'
import type { ScriptMusicEvent } from '../../../types'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class MusicProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'music'
  }

  async processEvent(event: ScriptMusicEvent): Promise<void> {
    const uiStore = useUIStore()

    let url = 'None'

    if (event.musicPath) {
      try {
        url = convertFileSrc(event.musicPath)
      } catch {
        url = 'None'
      }
    }

    uiStore.currentBackgroundMusic = url
  }
}

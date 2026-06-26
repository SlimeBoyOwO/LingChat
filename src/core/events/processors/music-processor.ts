import type { IEventProcessor } from '../event-processor'
import type { ScriptMusicEvent } from '../../../types'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class MusicProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'music'
  }

  async processEvent(event: ScriptMusicEvent): Promise<void> {
    const uiStore = useUIStore()

    // 存储原始文件路径，由 GameBackground.vue 统一做 convertFileSrc
    let url = 'None'

    if (event.musicPath) {
      url = event.musicPath
    }

    uiStore.currentBackgroundMusic = url
  }
}

import type { IEventProcessor } from '../event-processor'
import type { ScriptAmbientEvent } from '../../../types'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class AmbientProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'ambient'
  }

  async processEvent(event: ScriptAmbientEvent): Promise<void> {
    const uiStore = useUIStore()

    if (event.stop) {
      // 停止所有环境音
      if (event.ambientPath) {
        // 停止指定环境音
        uiStore.removeAmbientTrackBySrc(event.ambientPath)
      } else {
        // 停止所有环境音
        uiStore.clearAmbientTracks()
      }
      return
    }

    // 播放环境音 — 文件名去重
    uiStore.addAmbientTrack({
      src: event.ambientPath,
      volume: event.volume ?? 100,
      loop: event.loop ?? true,
      fade: event.fade ?? true,
    })
  }
}

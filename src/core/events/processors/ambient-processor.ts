/**
 * 环境音事件处理器
 * 处理剧本中的 ambient 事件，支持多轨并行环境音播放
 * 每轨独立音量控制，支持循环和淡出停止
 */
import type { IEventProcessor } from '../event-processor'
import type { ScriptAmbientEvent } from '../../../types'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class AmbientProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'ambient'
  }

  async processEvent(event: ScriptAmbientEvent): Promise<void> {
    const uiStore = useUIStore()

    // 停止指令：清除指定轨道或全部环境音
    if (event.stop) {
      uiStore.clearAmbientTracks(event.ambientPath || undefined)
      return
    }

    // 存储原始文件路径，由 GameBackground.vue 统一做 convertFileSrc
    if (!event.ambientPath) return

    // 添加环境音轨道到 store
    uiStore.addAmbientTrack({
      src: event.ambientPath,
      volume: event.volume ?? 100,
      loop: event.loop ?? true,
      fade: event.fade ?? true,
    })
  }
}

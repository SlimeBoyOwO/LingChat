import { invoke } from '@tauri-apps/api/core'
import type { IEventProcessor } from '../event-processor'
import type { ScriptConsoleWindowEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'

export default class ConsoleWindowProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'console_window'
  }

  async processEvent(event: ScriptConsoleWindowEvent, signal?: AbortSignal): Promise<void> {
    if (signal?.aborted) return
    useGameStore().currentStatus = 'presenting'
    try {
      // 这里只消费 Rust 校验并绑定当前剧本运行的一次性票据。
      await invoke('spawn_script_console_window', { requestId: event.requestId })
    } catch (error) {
      console.error('[ConsoleWindowProcessor] failed to spawn native system window:', error)
    }
  }
}

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
      // 在此队列位置才真正拉起真实系统控制台；文本由 Rust 命令侧再次净化
      await invoke('spawn_script_console_window', {
        title: event.title,
        text: event.text,
        count: event.count,
        interval: event.interval,
        lifetime: event.lifetime,
        style: event.style,
      })
    } catch (error) {
      console.error('[ConsoleWindowProcessor] failed to spawn console window:', error)
    }
  }
}

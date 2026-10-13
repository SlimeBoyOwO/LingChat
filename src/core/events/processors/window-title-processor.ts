import type { IEventProcessor } from '../event-processor'
import type { ScriptWindowTitleEvent } from '../../../types/script'
import { setScriptWindowTitle } from '@/utils/windowTitleCoordinator'

export default class WindowTitleProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'window_title'
  }

  async processEvent(event: ScriptWindowTitleEvent): Promise<void> {
    setScriptWindowTitle(event.title ?? '')
  }
}

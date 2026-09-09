import type { IEventProcessor } from '../event-processor'
import type { ScriptVoiceShiftEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class VoiceShiftProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'voice_shift'
  }

  async processEvent(event: ScriptVoiceShiftEvent): Promise<void> {
    useGameStore().currentStatus = 'presenting'
    const uiStore = useUIStore()
    uiStore.voiceRate = typeof event.rate === 'number' && event.rate > 0 ? event.rate : 1
    uiStore.voicePitch = typeof event.pitch === 'number' ? event.pitch : 0
  }
}

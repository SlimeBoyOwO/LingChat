import type { IEventProcessor } from '../event-processor'
import type { ScriptJumpscareEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class JumpscareProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'jumpscare'
  }

  async processEvent(event: ScriptJumpscareEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    gameStore.currentStatus = 'presenting'

    if (!event.imagePath) return
    // duration 缺省 0.6s：足够看清，又短到来不及躲开
    uiStore.triggerJumpscare(event.imagePath, event.soundPath || '', event.duration ?? 0.6)
  }
}

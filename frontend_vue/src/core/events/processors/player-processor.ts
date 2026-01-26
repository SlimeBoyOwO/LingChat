import type { IEventProcessor } from '../event-processor'
import type { ScriptPlayerEvent } from '../../../types'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class PlayerProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'player'
  }

  async processEvent(event: ScriptPlayerEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    // 更新游戏状态显示对话
    gameStore.currentStatus = 'responding'

    gameStore.appendGameMessage({
      type: 'message',
      displayName: gameStore.userName,
      content: event.text,
    })

    uiStore.showCharacterTitle = gameStore.userName
    uiStore.showCharacterSubtitle = gameStore.userSubtitle
    uiStore.showCharacterLine = event.text
    uiStore.showCharacterEmotion = event.emotion ? event.emotion : ''
  }
}

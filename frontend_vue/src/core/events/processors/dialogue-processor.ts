import type { IEventProcessor } from '../event-processor'
import type { ScriptDialogueEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'

export default class DialogueProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'reply'
  }

  async processEvent(event: ScriptDialogueEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    // 更新游戏状态显示对话
    gameStore.currentStatus = 'responding'

    // 针对剧本模式，获取角色
    const role = gameStore.getGameRole(event.roleId)
    if (!role) {
      console.warn('角色修改的角色似乎并没有被初始化')
      return
    }

    gameStore.currentLine = event.motionText
      ? `${event.message} (${event.motionText})`
      : event.message || ''

    gameStore.appendGameMessage({
      type: 'reply',
      displayName: role.roleName,
      content: event.message,
      emotion: event.emotion,
      audioFile: event.audioFile,
      isFinal: event.isFinal,
      motionText: event.motionText,
      originalTag: event.originalTag,
    })

    uiStore.showCharacterLine = gameStore.currentLine // TODO: 这部分逻辑之后整合
    role.emotion = event.emotion || '正常'
    role.originalEmotion = event.originalTag || '正常'
    uiStore.currentAvatarAudio = event.audioFile || 'None'
    uiStore.showCharacterEmotion = role.originalEmotion

    uiStore.showCharacterTitle = role.roleName
    uiStore.showCharacterSubtitle = role.roleSubTitle
    // gameStore.currentCharacter = event.character;

    // 对话总是等待用户继续，所以这里不需要做任何等待
    // event-queue 会自动检测到这是对话事件并等待用户继续
  }
}

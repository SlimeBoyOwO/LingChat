import type { IEventProcessor } from '../event-processor'
import type { ScriptModifyCharacterEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'

export default class ModifyCharacterProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'modify_character'
  }

  async processEvent(event: ScriptModifyCharacterEvent): Promise<void> {
    const gameStore = useGameStore()

    console.log('执行修改角色' + event.characterId + event.emotion + event.action)

    gameStore.currentStatus = 'presenting'

    if (event.characterId) {
      const role = gameStore.getGameRole(event.characterId)
      if (!role) {
        console.warn('角色修改的角色似乎并没有被初始化')
        return
      }

      if (event.action) {
        switch (event.action) {
          case 'show_character':
            role.show = true
            break
          case 'hide_character':
            role.show = false
            break
          default:
            break
        }
      }

      if (event.emotion) role.emotion = event.emotion
    } else console.warn('角色修改没有角色')

    // TODO: 根据查找的角色id，修改角色状态
  }
}

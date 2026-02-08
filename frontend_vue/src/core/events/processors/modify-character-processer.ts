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
    const delay = event.duration

    gameStore.currentStatus = 'presenting'

    if (event.characterId) {
      // 确保游戏初始化包含角色
      const role = await gameStore.getOrCreateGameRole(event.characterId)

      if (event.action) {
        switch (event.action) {
          case 'show_character':
            role.show = false // 确保之前是不显示的 TODO 不知道这个有没有必要加
            gameStore.presentRoleIds.push(event.characterId)
            role.show = true
            break
          case 'hide_character':
            role.show = false
            if (delay > 0) {
              setTimeout(() => {
                gameStore.presentRoleIds = gameStore.presentRoleIds.filter(
                  (id) => id !== event.characterId,
                )
              }, delay * 1000)
            } else {
              gameStore.presentRoleIds = gameStore.presentRoleIds.filter(
                (id) => id !== event.characterId,
              )
            }
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

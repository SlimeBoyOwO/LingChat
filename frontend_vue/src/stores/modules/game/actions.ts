// actions.ts
import type { GameState, GameMessage } from './state'
import { getGameInfo } from '../../../api/services/game-info'
import { useUIStore } from '../ui/ui'

export const actions = {
  // 注意：这里 this 指定为 GameState 是安全的，
  // 但如果你想调用 this.initializeGame()，TS 会报错。
  // 如果遇到这种情况，可以将 this 类型改为 any 或者手动定义完整 Store 接口。
  // TODO: 之后有扩展需求的时候，使用现代的 Pinia 的 Setup Store 模式

  appendGameMessage(this: GameState, message: GameMessage) {
    this.dialogHistory.push({
      ...message,
      timestamp: Date.now(),
    })
  },

  clearDialogHistory(this: GameState) {
    this.dialogHistory = []
  },

  async initializeGame(this: GameState, client_id: string, userId: string) {
    try {
      const gameInfo = await getGameInfo(client_id, userId)

      this.gameRoles = {}
      this.gameRoles[gameInfo.character_id] = {
        roleId: gameInfo.character_id,
        roleName: gameInfo.ai_name,
        roleSubTitle: gameInfo.ai_subtitle,
        thinkMessage: gameInfo.thinking_message,
        scale: gameInfo.scale,
        offsetX: gameInfo.offset_x,
        offsetY: gameInfo.offset_y,
        bubbleLeft: gameInfo.bubble_left,
        bubbleTop: gameInfo.bubble_top,
        clothes: gameInfo.clothes,
        clothesName: gameInfo.clothes_name,
        bodyPart: gameInfo.body_part,
        emotion: '正常',
        originalEmotion: '正常',
        show: true,
      }
      this.presentRoleIds.push(gameInfo.character_id)
      this.mainRoleId = gameInfo.character_id
      this.currentInteractRoleId = gameInfo.character_id

      const uiStore = useUIStore()
      this.userName = gameInfo.user_name
      this.userSubtitle = gameInfo.user_subtitle

      uiStore.showCharacterTitle = gameInfo.user_name
      uiStore.showCharacterSubtitle = gameInfo.user_subtitle

      return gameInfo
    } catch (error) {
      console.error('初始化游戏信息失败:', error)
      throw error
    }
  },
}

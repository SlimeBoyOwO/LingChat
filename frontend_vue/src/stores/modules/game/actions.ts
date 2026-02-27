// actions.ts
import type { GameState, GameMessage, GameRole } from './state'
import { getGameInfo } from '../../../api/services/game-info'
import { getRoleInfo } from '../../../api/services/character'
import { useUIStore } from '../ui/ui'
import { getDialogueHistory } from '@/api/services/history'
import { convertToGameMessages } from '@/utils/function'

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

  setGameMessages(this: GameState, messages: GameMessage[]) {
    this.dialogHistory = messages
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
      this.presentRoleIds = []
      this.presentRoleIds.push(gameInfo.character_id)
      this.mainRoleId = gameInfo.character_id
      this.currentInteractRoleId = gameInfo.character_id

      const uiStore = useUIStore()
      this.userName = gameInfo.user_name
      this.userSubtitle = gameInfo.user_subtitle

      uiStore.showCharacterTitle = gameInfo.user_name
      uiStore.showCharacterSubtitle = gameInfo.user_subtitle

      const lines = await getDialogueHistory(userId)
      if (lines && lines.length > 0) {
        const messages = convertToGameMessages(lines)
        this.dialogHistory = messages
      } else {
        this.dialogHistory = []
      }

      return gameInfo
    } catch (error) {
      console.error('初始化游戏信息失败:', error)
      throw error
    }
  },

  async getOrCreateGameRole(this: GameState, role_id: number): Promise<GameRole> {
    if (this.gameRoles[role_id]) {
      return this.gameRoles[role_id]
    }
    try {
      const roleInfo = await getRoleInfo(role_id)
      this.gameRoles[role_id] = {
        roleId: roleInfo.character_id,
        roleName: roleInfo.ai_name,
        roleSubTitle: roleInfo.ai_subtitle,
        thinkMessage: roleInfo.thinking_message,
        scale: roleInfo.scale,
        offsetX: roleInfo.offset_x,
        offsetY: roleInfo.offset_y,
        bubbleLeft: roleInfo.bubble_left,
        bubbleTop: roleInfo.bubble_top,
        clothes: roleInfo.clothes,
        clothesName: roleInfo.clothes_name,
        bodyPart: roleInfo.body_part,
        emotion: '正常',
        originalEmotion: '正常',
        show: true,
      }
      return this.gameRoles[role_id]
    } catch (error) {
      console.error('游戏角色信息获取失败:', error)
      throw error
    }
  },

  /** 标记进入剧情模式（用于控制UI显示：隐藏番茄钟/日程等） */
  enterStoryMode(this: GameState, scriptName: string = 'unknown') {
    this.runningScript = {
      scriptName,
      currentChapterName: '',
      isRunning: true,
    }
  },

  /** 标记退出剧情模式，回到自由对话模式 */
  exitStoryMode(this: GameState) {
    this.runningScript = null
  },
}

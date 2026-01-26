// getters.ts
import type { GameState, GameMessage, GameRole } from './state'

export const getters = {
  // 普通 Getter
  getCurrentLine(state: GameState): string {
    return state.currentLine
  },

  getDialogHistory(state: GameState): GameMessage[] {
    return state.dialogHistory
  },

  presentRolesList(state: GameState): GameRole[] {
    return state.presentRoleIds.map((id) => state.gameRoles[id]).filter((role) => !!role) // 过滤掉可能已被删除的角色
  },

  currentInteractRole(state: GameState): GameRole | undefined {
    return state.gameRoles[state.currentInteractRoleId]
  },

  // 获取主角对象
  mainRole(state: GameState): GameRole | undefined {
    return state.gameRoles[state.mainRoleId]
  },

  getGameRole: (state: GameState) => {
    return (roleId: number): GameRole | undefined => {
      return state.gameRoles[roleId]
    }
  },
}

import type { GameState, DialogMessage } from "./state";

export const getters = {
  getCurrentLine(state: GameState): string {
    return state.currentLine;
  },

  getDialogHistory(state: GameState): DialogMessage[] {
    return state.dialogHistory;
  },

  getAvatarInfo(state: GameState): { character_id: number; emotion: string } {
    return state.avatar;
  },

  getGameStatus(state: GameState): string {
    return state.currentStatus;
  },

  getCurrentScene(state: GameState): string {
    return state.currentScene;
  },
};

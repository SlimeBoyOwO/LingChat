import type { GameState, DialogMessage, ScriptCharacter } from "./state";

export const getters = {
  getCurrentLine(state: GameState): string {
    return state.currentLine;
  },

  getDialogHistory(state: GameState): DialogMessage[] {
    return state.dialogHistory;
  },

  getGameStatus(state: GameState): string {
    return state.currentStatus;
  },

  getCurrentScene(state: GameState): string {
    return state.currentScene;
  },

  // 辅助函数
  getCharacterByName(
    state: GameState,
    name: string
  ): ScriptCharacter | undefined {
    return state.script.script_characters.get(name);
  },
};

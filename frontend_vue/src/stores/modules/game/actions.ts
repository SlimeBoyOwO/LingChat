import type { GameState } from "./state";
import type { DialogMessage } from "./state";

export const actions = {
  updateLine(this: GameState, text: string): void {
    this.currentLine = text;
  },

  addDialogToHistory(this: GameState, ai_response: any[]): void {
    // TODO 添加AI回复组到历史中
  },

  setGameStatus(
    this: GameState,
    state: "input" | "thinking" | "responding"
  ): void {
    this.currentStatus = state;
  },

  addToDialogHistory(this: GameState, message: DialogMessage) {
    this.dialogHistory.push({
      ...message,
      timestamp: Date.now(),
    });
  },
  clearDialogHistory(this: GameState) {
    this.dialogHistory = [];
  },
  setCurrentStatus(this: GameState, status: GameState["currentStatus"]) {
    this.currentStatus = status;
  },
};

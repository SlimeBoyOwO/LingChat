import type { GameState } from "./state";
import type { DialogMessage, ScriptCharacter } from "./state";
import { getGameInfo } from "../../../api/services/game-info";
import { getScriptInfo } from "../../../api/services/script-info";
import { useUIStore } from "../ui/ui";

export const actions = {
  updateLine(this: GameState, text: string): void {
    this.currentLine = text;
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

  async initializeGame(this: GameState, userId: string) {
    try {
      const gameInfo = await getGameInfo(userId);

      // 更新 gameStore 自己的状态
      this.character = "default";
      this.avatar.character_name = gameInfo.ai_name;
      this.avatar.character_subtitle = gameInfo.ai_subtitle;
      this.avatar.user_name = gameInfo.user_name;
      this.avatar.user_subtitle = gameInfo.user_subtitle;
      this.avatar.character_id = gameInfo.character_id;
      this.avatar.think_message = gameInfo.thinking_message;
      this.avatar.scale = gameInfo.scale;
      this.avatar.offset_y = gameInfo.offset;
      this.avatar.bubble_left = gameInfo.bubble_left;
      this.avatar.bubble_top = gameInfo.bubble_top;

      // 也可以在这里直接更新其他 store 的状态
      const uiStore = useUIStore();
      uiStore.showCharacterTitle = gameInfo.user_name;
      uiStore.showCharacterSubtitle = gameInfo.user_subtitle;

      // 返回获取到的信息，方便组件进行UI操作
      return gameInfo;
    } catch (error) {
      console.error("初始化游戏信息失败:", error);
      // 抛出错误或返回 null，让调用方知道失败了
      throw error;
    }
  },

  async initializeScript(this: GameState, scriptName: string) {
    try {
      const scriptInfo = await getScriptInfo(scriptName);

      this.script.script_name = scriptInfo.script_name;
      this.script.script_characters.clear();

      Object.entries(scriptInfo.characters).forEach(
        ([characterId, characterData]) => {
          const scriptCharacter: ScriptCharacter = {
            character_id: characterId || 0,
            character_name: characterData.ai_name,
            character_subtitle: characterData.ai_subtitle,
            think_message: characterData.thinking_message,
            emotion: "正常",
            originEmotion: "",
            show: false,
            scale: characterData.scale,
            offset_y: characterData.offset_y,
            offset_x: characterData.offset_x,
            bubble_top: characterData.bubble_top,
            bubble_left: characterData.bubble_left,
          };

          // 使用 character_id 作为 key（转换为字符串确保一致性）
          this.script.script_characters.set(characterId, scriptCharacter);
        }
      );

      console.log(
        `脚本 "${this.script.script_name}" 初始化完成，共加载 ${this.script.script_characters.size} 个角色`
      );
    } catch (error) {
      console.error("初始化脚本失败:", error);
      throw error;
    }
  },
};

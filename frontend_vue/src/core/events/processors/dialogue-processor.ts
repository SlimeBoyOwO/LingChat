import { IEventProcessor } from "../event-processor";
import { ScriptDialogueEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";

export default class DialogueProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "reply";
  }

  async processEvent(event: ScriptDialogueEvent): Promise<void> {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    // 更新游戏状态显示对话
    gameStore.currentStatus = "responding";

    // 针对剧本模式，获取角色
    gameStore.character = gameStore.script.isRunning
      ? event.character
        ? event.character
        : "ERROR"
      : gameStore.avatar.character_name;

    gameStore.currentLine = event.motionText
      ? `${event.message} (${event.motionText})`
      : event.message || "";

    gameStore.addToDialogHistory({
      type: "reply",
      character: gameStore.script.isRunning
        ? event.character
          ? event.character
          : "ERROR"
        : gameStore.avatar.character_name,
      content: event.message,
      emotion: event.emotion,
      audioFile: event.audioFile,
      isFinal: event.isFinal,
      motionText: event.motionText,
      originalTag: event.originalTag,
    });

    uiStore.showCharacterLine = gameStore.currentLine; // TODO: 这部分逻辑之后整合
    gameStore.avatar.emotion = event.emotion || "正常";
    gameStore.avatar.originEmotion = event.originalTag || "正常";
    uiStore.currentAvatarAudio = event.audioFile || "None";
    uiStore.showCharacterEmotion = gameStore.avatar.originEmotion;

    uiStore.showCharacterTitle = gameStore.avatar.character_name;
    uiStore.showCharacterSubtitle = gameStore.avatar.character_subtitle;
    // gameStore.currentCharacter = event.character;

    // 对话总是等待用户继续，所以这里不需要做任何等待
    // event-queue 会自动检测到这是对话事件并等待用户继续
  }
}

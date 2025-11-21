import { IEventProcessor } from "../event-processor";
import { ScriptNarrationEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";

export default class NarrationProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "narration";
  }

  async processEvent(event: ScriptNarrationEvent): Promise<void> {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    // 更新游戏状态
    gameStore.currentStatus = "responding";
    uiStore.showCharacterLine = event.text;

    uiStore.showCharacterTitle = "";
    uiStore.showCharacterSubtitle = "";
    uiStore.showCharacterEmotion = "";

    gameStore.addToDialogHistory({
      type: "message",
      character: "旁白",
      content: event.text,
    });

    console.log("叙事模式执行" + event.text);
  }
}

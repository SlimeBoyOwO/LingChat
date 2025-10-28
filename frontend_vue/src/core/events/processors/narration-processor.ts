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

    console.log("叙事模式执行" + event.text);

    // 如果有duration，等待指定时间后自动继续
    if (event.duration) {
      await this.waitForDuration(event.duration);
    }
    // 如果没有duration，事件处理器完成，由event-queue等待用户继续
  }

  private waitForDuration(duration: number): Promise<void> {
    return new Promise((resolve) => {
      setTimeout(resolve, duration);
    });
  }
}

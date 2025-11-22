import { IEventProcessor } from "../event-processor";
import { ScriptBackgroundEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";

export default class BackgroundProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "background";
  }

  async processEvent(event: ScriptBackgroundEvent): Promise<void> {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    // 处理对话逻辑
    gameStore.currentStatus = "presenting";

    let url = event.imagePath
      ? `/api/v1/chat/script/background_file/${encodeURIComponent(
          event.imagePath
        )}`
      : "../pictures/background/default.png";

    uiStore.currentBackground = url;
  }
}

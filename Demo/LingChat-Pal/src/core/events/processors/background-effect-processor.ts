import { IEventProcessor } from "../event-processor";
import { ScriptBackgroundEffectEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";

export default class BackgroundEffectProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "background_effect";
  }

  async processEvent(event: ScriptBackgroundEffectEvent): Promise<void> {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    // 处理对话逻辑
    gameStore.currentStatus = "presenting";

    uiStore.currentBackgroundEffect = event.effect;
  }
}

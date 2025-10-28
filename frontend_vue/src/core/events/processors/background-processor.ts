import { IEventProcessor } from "../event-processor";
import { ScriptBackgroundEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";

export default class BackgroundProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "script_dialogue";
  }

  async processEvent(event: ScriptBackgroundEvent): Promise<void> {
    const gameStore = useGameStore();

    // 处理对话逻辑
    gameStore.currentStatus = "presenting";
    // 设置角色、文本等信息
  }
}

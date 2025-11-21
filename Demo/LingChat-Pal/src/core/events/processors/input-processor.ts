import { IEventProcessor } from "../event-processor";
import { ScriptInputEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";

export default class InputProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "input";
  }

  async processEvent(event: ScriptInputEvent): Promise<void> {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    // 更新游戏状态
    gameStore.currentStatus = "input";
    uiStore.showPlayerHintLine = event.hint;
    uiStore.showCharacterLine = "";

    console.log("玩家输入事件执行" + event.hint);
  }
}

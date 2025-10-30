import { IEventProcessor } from "../event-processor";
import { ScriptModifyCharacterEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";

export default class ModifyCharacterProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "modify_character";
  }

  async processEvent(event: ScriptModifyCharacterEvent): Promise<void> {
    const gameStore = useGameStore();

    console.log("执行修改角色" + event.character + event.emotion);
    // 更新游戏状态
    gameStore.currentStatus = "presenting";
    if (event.character) gameStore.character = event.character;
    else console.warn("角色修改没有角色");
    if (event.emotion) gameStore.avatar.emotion = event.emotion;
  }
}

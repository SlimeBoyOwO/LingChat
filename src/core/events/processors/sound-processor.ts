import type { IEventProcessor } from "../event-processor";
import type { ScriptSoundEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";

export default class SoundProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "sound";
  }

  async processEvent(event: ScriptSoundEvent): Promise<void> {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    gameStore.currentStatus = "presenting";

    // 存储原始文件路径并递增播放序号；相同故障音连续出现也必须重播。
    uiStore.triggerSoundEffect(event.soundPath || "None");
  }
}

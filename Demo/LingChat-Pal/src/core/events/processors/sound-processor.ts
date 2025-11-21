import { IEventProcessor } from "../event-processor";
import { ScriptSoundEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";

export default class SoundProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "sound";
  }

  async processEvent(event: ScriptSoundEvent): Promise<void> {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    // 处理对话逻辑
    gameStore.currentStatus = "presenting";

    let url = event.soundPath
      ? `/api/v1/chat/script/sound_file/${encodeURIComponent(event.soundPath)}`
      : "None";

    uiStore.currentSoundEffect = url;
  }
}

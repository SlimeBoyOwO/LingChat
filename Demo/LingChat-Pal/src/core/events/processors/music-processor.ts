import { IEventProcessor } from "../event-processor";
import { ScriptMusicEvent } from "../../../types";
import { useUIStore } from "../../../stores/modules/ui/ui";

export default class MusicProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "music";
  }

  async processEvent(event: ScriptMusicEvent): Promise<void> {
    const uiStore = useUIStore();

    let url = event.musicPath
      ? `/api/v1/chat/script/music_file/${encodeURIComponent(event.musicPath)}`
      : "None";

    uiStore.currentBackgroundMusic = url;
  }
}

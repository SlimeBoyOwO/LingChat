import { invoke } from "@tauri-apps/api/core";
import type { IEventProcessor } from "@lingchat/host/core/events/event-processor";
import type { ScriptGlitchWindowEvent } from "@lingchat/host/types";
import { useGameStore } from "@lingchat/host/stores/modules/game";

export default class GlitchWindowProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "glitch_window";
  }

  async processEvent(event: ScriptGlitchWindowEvent, signal?: AbortSignal): Promise<void> {
    if (signal?.aborted) return;
    useGameStore().currentStatus = "presenting";
    try {
      await invoke("show_script_glitch_window", { requestId: event.requestId });
    } catch (error) {
      // Missing/consumed tickets are non-fatal (for example after an explicit
      // stop). The story queue must remain usable even when a window is skipped.
      console.error("[GlitchWindowProcessor] failed to show validated window:", error);
    }
  }
}

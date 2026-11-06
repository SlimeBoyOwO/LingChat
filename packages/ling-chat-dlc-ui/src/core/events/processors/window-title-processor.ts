import type { IEventProcessor } from "@lingchat/host/core/events/event-processor";
import type { ScriptWindowTitleEvent } from "@lingchat/host/types/script";
import { setScriptWindowTitle } from "../../../utils/windowTitleCoordinator.ts";

export default class WindowTitleProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "window_title";
  }

  async processEvent(event: ScriptWindowTitleEvent): Promise<void> {
    setScriptWindowTitle(event.title ?? "");
  }
}

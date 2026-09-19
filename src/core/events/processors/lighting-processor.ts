import type { IEventProcessor } from "../event-processor";
import type { ScriptLightingEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useLightingStore } from "../../../stores/modules/lighting";

/**
 * 剧本 lighting 事件处理器。
 *
 * 走事件队列而不是即时监听，光影才跟着台词节奏切换——否则剧本一执行，
 * 灯光就在第一句台词还没打完时跳走了。
 */
export default class LightingProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "lighting";
  }

  async processEvent(event: ScriptLightingEvent): Promise<void> {
    const gameStore = useGameStore();
    const lightingStore = useLightingStore();

    gameStore.currentStatus = "presenting";

    lightingStore.applyPayload({
      preset: event.preset,
      params: event.params,
      source: event.source || "script",
    });
  }
}

import type { IEventProcessor } from "../event-processor";
import type { ScriptProgressEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { WebSocketMessageTypes } from "../../../types";

export default class ProgressProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === WebSocketMessageTypes.SCRIPT_PROGRESS;
  }

  async processEvent(event: ScriptProgressEvent): Promise<void> {
    const gameStore = useGameStore();

    // 队列按阅读速度消费，播到锚点即玩家刚读完它之前的台词，
    // 因此这里记录的就是当前阅读位置，存档时据此还原精确恢复点。
    gameStore.scriptReadCursor = {
      chapter: event.chapter,
      eventIndex: event.event_index,
      lineCount: event.line_count,
      vars: event.vars,
    };
  }
}

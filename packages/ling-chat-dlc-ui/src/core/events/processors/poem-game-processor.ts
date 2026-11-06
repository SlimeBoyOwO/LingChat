import type { IEventProcessor } from "@lingchat/host/core/events/event-processor";
import type { ScriptPoemGameEvent } from "@lingchat/host/types";
import { useGameStore } from "@lingchat/host/stores/modules/game";

export default class PoemGameProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "poem_game";
  }

  async processEvent(event: ScriptPoemGameEvent): Promise<void> {
    const gameStore = useGameStore();
    gameStore.poemGame = event;
    gameStore.currentStatus = "input";
  }
}

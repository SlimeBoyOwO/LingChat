import { registerHandler, sendWebSocketChatMessage } from "..";
import { WebSocketMessageTypes } from "../../../types";
import { eventQueue } from "../../../core/events/event-queue";
import { useGameStore } from "../../../stores/modules/game";
import {
  ScriptNarrationEvent,
  ScriptBackgroundEvent,
  ScriptDialogueEvent,
  ScriptPlayerEvent,
} from "../../../types";

export class ScriptHandler {
  constructor() {
    this.registerHandlers();
  }

  private registerHandlers() {
    registerHandler(WebSocketMessageTypes.SCRIPT_NARRATION, (data: any) => {
      console.log("收到剧本旁白事件:", data);
      eventQueue.addEvent(data as ScriptNarrationEvent);
    });

    registerHandler(WebSocketMessageTypes.SCRIPT_DIALOGUE, (data: any) => {
      console.log("收到剧本对话事件:", data);
      eventQueue.addEvent(data as ScriptDialogueEvent);
    });

    registerHandler(WebSocketMessageTypes.SCRIPT_BACKGROUND, (data: any) => {
      console.log("收到背景切换事件:", data);
      eventQueue.addEvent(data as ScriptBackgroundEvent);
    });

    registerHandler(WebSocketMessageTypes.SCRIPT_PLAYER, (data: any) => {
      console.log("收到主人公对话事件:", data);
      eventQueue.addEvent(data as ScriptPlayerEvent);
    });
  }

  public sendMessage(text: string) {
    const gameStore = useGameStore();

    if (!text.trim()) return;

    gameStore.currentStatus = "thinking";
    gameStore.addToDialogHistory({
      type: "message",
      content: text,
    });

    sendWebSocketChatMessage(WebSocketMessageTypes.MESSAGE, text);
  }
}

// 导出单例
export const scriptHandler = new ScriptHandler();

import { registerHandler, sendWebSocketChatMessage } from "..";
import { WebSocketMessageTypes } from "../../../types";
import { eventQueue } from "../../../core/events/event-queue";
import type * as ScriptTypes from "../../../types/script";

export class ScriptHandler {
  constructor() {
    this.registerHandlers();
  }

  private registerHandlers() {
    registerHandler(WebSocketMessageTypes.SCRIPT_NARRATION, (data: any) => {
      console.log("收到剧本旁白事件:", data);
      eventQueue.addEvent(data as ScriptTypes.ScriptNarrationEvent);
    });

    registerHandler(WebSocketMessageTypes.SCRIPT_DIALOGUE, (data: any) => {
      const timestamp = new Date().toISOString();
      console.log(`[${timestamp}] 收到剧本对话事件:`, data);
      console.log(`[${timestamp}] 添加到事件队列，当前队列长度:`, eventQueue.getState().queueLength);
      eventQueue.addEvent(data as ScriptTypes.ScriptDialogueEvent);
    });

    registerHandler(WebSocketMessageTypes.SCRIPT_BACKGROUND, (data: any) => {
      console.log("收到背景切换事件:", data);
      eventQueue.addEvent(data as ScriptTypes.ScriptBackgroundEvent);
    });

    registerHandler(WebSocketMessageTypes.SCRIPT_PLAYER, (data: any) => {
      console.log("收到主人公对话事件:", data);
      eventQueue.addEvent(data as ScriptTypes.ScriptPlayerEvent);
    });

    registerHandler(WebSocketMessageTypes.SCRIPT_MUSIC, (data: any) => {
      console.log("收到背景音乐切换事件:", data);
      eventQueue.addEvent(data as ScriptTypes.ScriptMusicEvent);
    });

    registerHandler(
      WebSocketMessageTypes.SCRIPT_BACKGROUND_EFFECT,
      (data: any) => {
        console.log("收到背景特效切换事件:", data);
        eventQueue.addEvent(data as ScriptTypes.ScriptBackgroundEffectEvent);
      }
    );

    registerHandler(WebSocketMessageTypes.SCRIPT_SOUND, (data: any) => {
      console.log("收到音效切换事件:", data);
      eventQueue.addEvent(data as ScriptTypes.ScriptSoundEvent);
    });

    registerHandler(
      WebSocketMessageTypes.SCRIPT_MODIFY_CHARACTER,
      (data: any) => {
        console.log("收到修改角色事件:", data);
        eventQueue.addEvent(data as ScriptTypes.ScriptModifyCharacterEvent);
      }
    );

    registerHandler(WebSocketMessageTypes.SCRIPT_INPUT, (data: any) => {
      console.log("收到输入事件:", data);
      eventQueue.addEvent(data as ScriptTypes.ScriptInputEvent);
    });
  }

  public sendMessage(text: string) {
    if (!text.trim()) return;
    sendWebSocketChatMessage(WebSocketMessageTypes.MESSAGE, text);
  }
}

// 导出单例
export const scriptHandler = new ScriptHandler();

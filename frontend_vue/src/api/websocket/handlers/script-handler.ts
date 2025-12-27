import { registerHandler, sendWebSocketChatMessage } from '..'
import { WebSocketMessageTypes } from '../../../types'
import { eventQueue } from '../../../core/events/event-queue'
import { useUserStore } from '../../../stores/modules/user/user'
import type * as ScriptTypes from '../../../types/script'

export class ScriptHandler {
  constructor() {
    this.registerHandlers()
  }

  private registerHandlers() {
    registerHandler(WebSocketMessageTypes.CONNECTION, (data: any) => {
      console.log('收到链接建立事件:', data)
      useUserStore().client_id = data.client_id // 保存client_id
    })

    registerHandler(WebSocketMessageTypes.SCRIPT_NARRATION, (data: any) => {
      console.log('收到剧本旁白事件:', data)
      eventQueue.addEvent(data as ScriptTypes.ScriptNarrationEvent)
    })

    registerHandler(WebSocketMessageTypes.SCRIPT_DIALOGUE, (data: any) => {
      console.log('收到剧本对话事件:', data)
      eventQueue.addEvent(data as ScriptTypes.ScriptDialogueEvent)
    })

    registerHandler(WebSocketMessageTypes.SCRIPT_BACKGROUND, (data: any) => {
      console.log('收到背景切换事件:', data)
      eventQueue.addEvent(data as ScriptTypes.ScriptBackgroundEvent)
    })

    registerHandler(WebSocketMessageTypes.SCRIPT_PLAYER, (data: any) => {
      console.log('收到主人公对话事件:', data)
      eventQueue.addEvent(data as ScriptTypes.ScriptPlayerEvent)
    })

    registerHandler(WebSocketMessageTypes.SCRIPT_MUSIC, (data: any) => {
      console.log('收到背景音乐切换事件:', data)
      eventQueue.addEvent(data as ScriptTypes.ScriptMusicEvent)
    })

    registerHandler(WebSocketMessageTypes.SCRIPT_BACKGROUND_EFFECT, (data: any) => {
      console.log('收到背景特效切换事件:', data)
      eventQueue.addEvent(data as ScriptTypes.ScriptBackgroundEffectEvent)
    })

    registerHandler(WebSocketMessageTypes.SCRIPT_SOUND, (data: any) => {
      console.log('收到音效切换事件:', data)
      eventQueue.addEvent(data as ScriptTypes.ScriptSoundEvent)
    })

    registerHandler(WebSocketMessageTypes.SCRIPT_MODIFY_CHARACTER, (data: any) => {
      console.log('收到修改角色事件:', data)
      eventQueue.addEvent(data as ScriptTypes.ScriptModifyCharacterEvent)
    })

    registerHandler(WebSocketMessageTypes.SCRIPT_INPUT, (data: any) => {
      console.log("收到输入事件:", data);
      eventQueue.addEvent(data as ScriptTypes.ScriptInputEvent);
    });

    // 注册错误处理器 - 显示后端LLM错误
    registerHandler(WebSocketMessageTypes.ERROR, (data: any) => {
      console.log("收到错误消息:", data);

      // 动态导入通知composable并显示错误
      import("../../../composables/ui/useNotification").then(
        ({ useNotification }) => {
          const { showError } = useNotification();

          // 使用 error_code 查询对应的角色专属提示
          showError({
            errorCode: data.error_code || 'default_error',
          });
        }
      );

      // 同时重置游戏状态（不等待后端 status_reset）
      import("../../../stores/modules/game").then(({ useGameStore }) => {
        const gameStore = useGameStore();
        gameStore.currentStatus = "input";
        gameStore.currentLine = "";
        console.log("游戏状态已重置为: input (由错误处理器触发)");
      });
    });

    // 注册状态重置处理器 - 让对话框回到输入状态（后端也可能发送）
    registerHandler("status_reset", (data: any) => {
      console.log("收到状态重置消息:", data);

      // 动态导入game store并重置状态
      import("../../../stores/modules/game").then(({ useGameStore }) => {
        const gameStore = useGameStore();
        gameStore.currentStatus = data.status || "input";
        gameStore.currentLine = ""; // 清空当前对话内容
        console.log("游戏状态已重置为:", data.status);
      });
    });
  }

  public sendMessage(text: string) {
    if (!text.trim()) return
    sendWebSocketChatMessage(WebSocketMessageTypes.MESSAGE, text)
  }
}

// 导出单例
export const scriptHandler = new ScriptHandler()

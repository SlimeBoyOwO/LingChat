import { registerHandler, sendWebSocketChatMessage } from '..'
import { WebSocketMessageTypes } from '../../../types'
import { eventQueue } from '../../../core/events/event-queue'
import { useUserStore } from '../../../stores/modules/user/user'
import { useGameStore } from '../../../stores/modules/game'
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
      console.log('收到输入事件:', data)
      eventQueue.addEvent(data as ScriptTypes.ScriptInputEvent)
    })

    // 错误事件 - 通过 eventQueue 统一处理
    registerHandler(WebSocketMessageTypes.ERROR, (data: any) => {
      console.log('收到错误消息:', data)
      eventQueue.addEvent({ ...data, type: 'error', duration: 0 } as ScriptTypes.ScriptErrorEvent)
    })

    // 状态重置事件 - 通过 eventQueue 统一处理
    registerHandler('status_reset', (data: any) => {
      console.log('收到状态重置消息:', data)
      eventQueue.addEvent({
        ...data,
        type: 'status_reset',
        duration: 0,
      } as ScriptTypes.ScriptStatusResetEvent)
    })
  }

  public sendMessage(text: string, instruction?: string) {
    if (!text.trim()) return
    const message = instruction ? `${text}[!Temp!]${instruction}[/!Temp!]` : text

    // 用户在任意位置输入 /开始剧本 时，也标记为剧情模式（用于隐藏番茄钟/日程等自由模式工具）
    if (text.trim().startsWith('/开始剧本')) {
      const parts = text.trim().split(/\s+/, 2)
      const scriptName = parts[1] || 'default'
      useGameStore().enterStoryMode(scriptName)
    }

    sendWebSocketChatMessage(WebSocketMessageTypes.MESSAGE, message)
  }
}

// 导出单例
export const scriptHandler = new ScriptHandler()

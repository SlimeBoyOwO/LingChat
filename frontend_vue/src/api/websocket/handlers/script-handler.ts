// scriptHandler.ts
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { registerHandler } from "..";
import { WebSocketMessageTypes } from "../../../types";

export interface ScriptEvent {
  type: string;
  isFinal?: boolean;
  // 其他通用字段...
}

export interface ScriptNarrationEvent extends ScriptEvent {
  type: "script_narration";
  text: string;
  duration?: number; // 显示时长(ms)
  sceneId?: string;
}

export interface ScriptDialogueEvent extends ScriptEvent {
  type: "script_dialogue";
  character: string;
  text: string;
  emotion?: string;
  audioFile?: string;
  motionText?: string;
}

export interface ScriptBackgroundEvent extends ScriptEvent {
  type: "script_background";
  imagePath: string;
  transition?: string;
  duration?: number;
}

export type ScriptEventType =
  | ScriptNarrationEvent
  | ScriptDialogueEvent
  | ScriptBackgroundEvent;

export class ScriptHandler {
  // 事件队列
  private eventQueue: ScriptEventType[] = [];
  // 当前正在处理的事件
  private currentEvent: ScriptEventType | null = null;
  // 是否正在处理中
  private isProcessing = false;

  constructor() {
    this.registerHandlers();
  }

  private registerHandlers() {
    // 注册所有剧本事件处理器
    registerHandler(WebSocketMessageTypes.SCRIPT_NARRATION, (data: any) => {
      console.log("收到剧本旁白事件:", data);
      this.addToQueue(data as ScriptNarrationEvent);
    });

    registerHandler(WebSocketMessageTypes.SCRIPT_DIALOGUE, (data: any) => {
      console.log("收到剧本对话事件:", data);
      this.addToQueue(data as ScriptDialogueEvent);
    });

    registerHandler(WebSocketMessageTypes.SCRIPT_BACKGROUND, (data: any) => {
      console.log("收到背景切换事件:", data);
      this.addToQueue(data as ScriptBackgroundEvent);
    });
  }

  /**
   * 添加事件到队列
   */
  private addToQueue(event: ScriptEventType) {
    this.eventQueue.push(event);
    if (!this.isProcessing) {
      this.processNextEvent();
    }
  }

  /**
   * 处理下一个事件
   */
  private processNextEvent() {
    // 如果剧本的接受事件长度结束了，说明是在等待玩家输入的状态了，重置UI状态到input
    if (this.eventQueue.length === 0) {
      this.isProcessing = false;
      this.resetConversationState();
      return;
    }

    this.isProcessing = true;
    this.currentEvent = this.eventQueue.shift() || null;

    if (!this.currentEvent) {
      this.isProcessing = false;
      return;
    }

    // 根据事件类型分发处理
    switch (this.currentEvent.type) {
      case "script_narration":
        this.handleNarration(this.currentEvent as ScriptNarrationEvent);
        break;
      case "script_dialogue":
        // this.handleDialogue(this.currentEvent as ScriptDialogueEvent);
        break;
      case "script_background":
        // this.handleBackground(this.currentEvent as ScriptBackgroundEvent);
        break;
      default:
        console.warn("未知的剧本事件类型:", this.currentEvent);
        this.completeCurrentEvent();
    }
  }

  /**
   * 处理旁白事件
   */
  private handleNarration(event: ScriptNarrationEvent) {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    // 更新游戏状态
    gameStore.currentStatus = "narrating";
    gameStore.currentLine = event.text;

    // TODO: 添加到对话历史
  }

  /**
   * 完成当前事件，处理下一个
   */
  private completeCurrentEvent() {
    this.currentEvent = null;
    this.processNextEvent();
  }

  /**
   * 用户继续（用于等待用户输入的情况）
   */
  public continueScript() {
    const gameStore = useGameStore();

    this.completeCurrentEvent();
  }

  /**
   * 清空队列（用于中断剧本）
   */
  public clearQueue() {
    this.eventQueue = [];
    this.currentEvent = null;
    this.isProcessing = false;

    const gameStore = useGameStore();
    gameStore.currentStatus = "input";
  }

  /**
   * 获取队列状态
   */
  public getQueueState() {
    return {
      queueLength: this.eventQueue.length,
      isProcessing: this.isProcessing,
      currentEvent: this.currentEvent,
    };
  }

  private resetConversationState() {
    const gameStore = useGameStore();

    this.isProcessing = false;
    gameStore.currentStatus = "input";
    gameStore.currentLine = "";
  }
}

// 导出单例
export const scriptHandler = new ScriptHandler();

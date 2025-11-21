import { ScriptEventType } from "../../types";
import { eventProcessorManager } from "./event-processor";
import { useGameStore } from "../../stores/modules/game";

export class EventQueue {
  private queue: ScriptEventType[] = [];
  private isProcessing = false;
  private currentEvent: ScriptEventType | null = null;
  private currentResolve: (() => void) | null = null;

  addEvent(event: ScriptEventType) {
    this.queue.push(event);
    if (!this.isProcessing) {
      this.processQueue();
    }
  }

  private async processQueue() {
    this.isProcessing = true;

    while (this.queue.length > 0) {
      const event = this.queue.shift();
      if (event) {
        this.currentEvent = event;
        await this.processSingleEvent(event);
      }
    }

    this.isProcessing = false;
    if (this.currentEvent?.isFinal) {
      this.resetToInputState();
    }
  }

  private async processSingleEvent(event: ScriptEventType): Promise<void> {
    // 处理事件并等待完成
    await eventProcessorManager.processEvent(event);

    // 如果事件需要等待用户继续，就等待
    if (this.shouldWaitForUser(event)) {
      await this.waitForUserContinue();
    } else {
      await this.waitForDuration(event.duration);
      console.log("等待" + event.duration + "秒");
    }
  }

  private shouldWaitForUser(event: ScriptEventType): boolean {
    // 明确检查 duration 是否为 null 或 undefined
    if (event.duration === null || event.duration === undefined) {
      return true; // 没有设置 duration，等待用户
    }

    // duration 为负数时等待用户
    if (event.duration < 0) {
      return true;
    }

    // duration 为 0 或正数时，不等待用户
    return false;
  }

  private waitForUserContinue(): Promise<void> {
    return new Promise((resolve) => {
      this.currentResolve = resolve;
      // 设置游戏状态为等待用户输入
      const gameStore = useGameStore();
      gameStore.currentStatus = "responding";
    });
  }

  // 用户继续的方法
  public continue(): boolean {
    let needWait = false; // 这个用于标记下个消息是否还没到来，要想继续还需要等待的信号

    if (this.currentResolve) {
      this.currentResolve();
      this.currentResolve = null;
    }

    // 假如当前消息不是最后一个，但是队列事件已经没了
    if (!this.currentEvent?.isFinal && this.queue.length === 0) {
      needWait = true;
      console.log("后面的消息还没到，请稍等");
    }

    return needWait;
  }

  clear() {
    this.queue = [];
    this.isProcessing = false;
    this.currentResolve = null;
    this.resetToInputState();
  }

  private resetToInputState() {
    this.currentEvent = null;

    const gameStore = useGameStore();
    gameStore.currentStatus = "input";
    gameStore.currentLine = "";
  }

  getState() {
    return {
      queueLength: this.queue.length,
      isProcessing: this.isProcessing,
      isWaitingForUser: this.currentResolve !== null,
    };
  }

  private waitForDuration(duration: number): Promise<void> {
    return new Promise((resolve) => {
      setTimeout(resolve, duration * 1000);
    });
  }
}

export const eventQueue = new EventQueue();

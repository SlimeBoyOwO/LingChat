import { useGameStore } from "../../stores/modules/game";
import { useSettingsStore } from "../../stores/modules/settings";
import { useUIStore } from "../../stores/modules/ui/ui";
import type { ScriptEventType } from "../../types";
import { dialogueMerge } from "./dialogue-merge";
import { eventProcessorManager } from "./event-processor";

export class EventQueue {
  private queue: ScriptEventType[] = [];
  private isProcessing = false;
  private paused = true;
  private currentEvent: ScriptEventType | null = null;
  private currentResolve: (() => void) | null = null;

  addEvent(event: ScriptEventType) {
    if ((event.type === "error" || event.type === "status_reset") && this.currentResolve) {
      this.currentResolve();
      this.currentResolve = null;
      this.queue = [];
      dialogueMerge.armed = false;
    }

    // 台词合并判定：新 reply 到达时，若当前展示的 i 句仍在展示、同角色、够短、
    // 内联模式开启、且队列无挡路事件（i+1 就是下一条要处理的）→ 武装，
    // 由 MainChat 在 i 展示完成时自动续打（merge 优先于 AUTO）、GameDialog 走追加路径。
    // 「i 仍在展示」的判据：
    //   - 非 AUTO：只看瞬时打字/音频状态（i 展示期间到达才合并）——保持原语义。
    //   - AUTO：放宽为「队列正停着等推进」（isWaitingForUser，currentResolve 挂起）。
    //     AUTO 的推进延迟可调、AI 连发时 i+1 常落在 i 已展示完、AUTO 还没推进的窗口，
    //     此时仍应合并。isWaitingForUser 在 i 展示期间到被推进前恒为 true，比瞬时状态稳。
    // 不含用户点击（点击跳过后 isTyping 已变 false，但武装已在到达时定格）。
    if (event.type === "reply") {
      const settings = useSettingsStore();
      const gameStore = useGameStore();
      const uiStore = useUIStore();
      const stillShowing = uiStore.autoMode
        ? this.getState().isWaitingForUser
        : dialogueMerge.isTyping || dialogueMerge.isAudioPlaying;
      if (
        settings.text.inlineMotionText &&
        settings.text.mergeLineThreshold > 0 &&
        stillShowing &&
        event.roleId === gameStore.currentInteractRoleId &&
        dialogueMerge.mergedLength + event.message.length <= settings.text.mergeLineThreshold &&
        !this.queue.some((e) => e.type !== "thinking")
      ) {
        dialogueMerge.armed = true;
        dialogueMerge.armedRoleId = event.roleId;
      }
    }

    this.queue.push(event);
    if (!this.isProcessing && !this.paused) {
      this.processQueue();
    }
  }

  private async processQueue() {
    this.isProcessing = true;
    try {
      while (this.queue.length > 0) {
        const event = this.queue.shift();
        if (event) {
          // 如果当前事件是thinking类型，且队列后面还有别的事件，则跳过
          if (event.type === "thinking" && this.queue.length > 0) {
            continue;
          }
          this.currentEvent = event;
          try {
            await this.processSingleEvent(event);
          } catch (error) {
            console.error("处理事件失败:", error, event);
            this.resetToInputState();
          }
        }
      }
    } finally {
      this.isProcessing = false;
      if (this.currentEvent?.isFinal) {
        this.resetToInputState();
      }
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
      console.log("后面的消息还没到，请稍等，最后一个消息是:", this.currentEvent);
    }

    return needWait;
  }

  clear() {
    this.queue = [];
    this.isProcessing = false;
    this.paused = true;
    this.currentResolve = null;
    this.resetToInputState();
  }

  /** 恢复事件队列消费（MainChat 就绪后调用） */
  resume() {
    this.paused = false;
    if (this.queue.length > 0 && !this.isProcessing) {
      this.processQueue();
    }
  }

  private resetToInputState() {
    this.currentEvent = null;
    dialogueMerge.armed = false;

    const gameStore = useGameStore();
    gameStore.currentStatus = "input";
    gameStore.currentLine = "";
  }

  /** 查看队头「下一个要处理」的事件（跳过 thinking），不消费队列。 */
  peek(): ScriptEventType | null {
    return this.queue.find((e) => e.type !== "thinking") ?? null;
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

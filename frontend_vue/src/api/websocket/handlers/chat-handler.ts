import { useGameStore } from "../../../stores/modules/game";
import { WebSocketChatMessage, WebSocketMessageTypes } from "../../../types";
import { registerHandler, sendWebSocketChatMessage } from "..";
import { useUIStore } from "../../../stores/modules/ui/ui";

export class ChatHandler {
  // 消息队列
  private messageQueue: WebSocketChatMessage[] = [];
  // 当前消息
  private currentMessagePart: WebSocketChatMessage | null = null;
  // 是否正在回复中
  private isProcessing = false;

  constructor() {
    this.registerHandlers();
  }

  private registerHandlers() {
    registerHandler(WebSocketMessageTypes.AIREPLY, (data: any) => {
      console.log(data);
      this.handleAIReply(data as WebSocketChatMessage);
    });

    registerHandler(WebSocketMessageTypes.SCRIPT_NARRATION, (data: any) => {
      console.log(data);
      // TODO: 实现旁白消息接受机制 this.handleSystemNarration(data as WebSocketChatMessage);
    });
  }

  private handleAIReply(data: WebSocketChatMessage) {
    const gameStore = useGameStore();

    try {
      this.handleChatMessage(data);
    } catch (error) {
      console.error("处理回复消息出错:", error);
      gameStore.currentStatus = "input";
    }
  }

  private handleChatMessage(data: WebSocketChatMessage) {
    this.messageQueue.push(data);
    if (!this.isProcessing) {
      this.continueMessage();
    }
  }

  public continueMessage(): boolean {
    let needWait = false; // 这个用于标记下个消息是否还没到来，要想继续还需要等待的信号
    if (this.currentMessagePart?.isFinal) {
      this.resetConversationState();
      if (this.messageQueue.length > 0) {
        // 假如说消息队列还有新来的消息（后台主动发送的消息） 则等待一段时间后立马变成消息回复状态
        setTimeout(() => {
          this.processNextMessage();
        }, 2000);
      }
    } else {
      if (this.messageQueue.length === 0) {
        // 如果这个消息并非最后一个，但后面的消息还没到，则通知needWait
        // TODO: 这里增加一些给前端UI的提示，告诉用户要等一下别急
        needWait = true;
        console.log("后面的消息还没到，请稍等");
      } else {
        this.processNextMessage();
      }
    }
    return needWait;
  }

  private processNextMessage() {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    gameStore.currentStatus = "responding"; // TODO: 回复阶段，就一直切换到responding模式，防止bug，但具体还需要验证

    this.isProcessing = true;

    // 从消息队列出队
    this.currentMessagePart = this.messageQueue.shift() || null;

    if (!this.currentMessagePart) return;

    // 显示展示
    const displayText = this.currentMessagePart.motionText
      ? `${this.currentMessagePart.message}（${this.currentMessagePart.motionText}）`
      : this.currentMessagePart.message || "";

    const isFinal = this.currentMessagePart.isFinal;

    gameStore.addToDialogHistory({
      type: "reply",
      content: this.currentMessagePart.message,
      emotion: this.currentMessagePart.emotion,
      audioFile: this.currentMessagePart.audioFile,
      isFinal,
      motionText: this.currentMessagePart.motionText,
      originalTag: this.currentMessagePart.originalTag,
    });

    gameStore.currentLine = displayText;
    gameStore.avatar.emotion = this.currentMessagePart.emotion || "正常";
    gameStore.avatar.originEmotion =
      this.currentMessagePart.originalTag || "正常";
    uiStore.currentAvatarAudio = this.currentMessagePart.audioFile || "None";

    // UI中粉色情绪展示内容
    uiStore.showCharacterEmotion = gameStore.avatar.originEmotion;
  }

  public sendMessage(text: string) {
    const gameStore = useGameStore();

    if (!text.trim()) return;

    gameStore.currentStatus = "thinking";
    gameStore.addToDialogHistory({
      type: "message",
      content: text,
    });

    this.messageQueue = [];
    this.isProcessing = false;

    sendWebSocketChatMessage(WebSocketMessageTypes.MESSAGE, text);
  }

  private resetConversationState() {
    const gameStore = useGameStore();

    this.currentMessagePart = null;
    // this.messageQueue = [];
    this.isProcessing = false;
    gameStore.currentStatus = "input";
    gameStore.currentLine = "";
  }
}

// 导出单例
export const chatHandler = new ChatHandler();

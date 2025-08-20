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
    registerHandler(WebSocketMessageTypes.REPLY, (data: any) => {
      console.log(data);
      this.handleReply(data as WebSocketChatMessage);
    });
  }

  private handleReply(data: WebSocketChatMessage) {
    const gameStore = useGameStore();

    try {
      this.handleChatMessage(data);
      gameStore.currentStatus = "responding";
    } catch (error) {
      console.error("处理回复消息出错:", error);
      gameStore.currentStatus = "input";
    }
  }

  private handleChatMessage(data: WebSocketChatMessage) {
    this.messageQueue.push(data);
    if (!this.isProcessing) {
      this.continueMessage();
      this.isProcessing = true;
    }
  }

  public continueMessage() {
    if (this.currentMessagePart?.isFinal) {
      // TODO 假如说消息队列还有新来的消息（后台主动发送的消息） 则等待一段时间后立马变成消息回复状态
      this.resetConversationState();
    } else {
      this.processNextMessage();
    }
  }

  private processNextMessage() {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

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
    uiStore.currentAvatarAudio = this.currentMessagePart.audioFile || "None";

    // UI中粉色情绪展示内容
    uiStore.showCharacterEmotion = gameStore.avatar.emotion;
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

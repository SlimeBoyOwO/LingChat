import { useGameStore } from "../../../stores/modules/game";
import { WebSocketChatMessage, WebSocketMessageTypes } from "../../../types";
import { registerHandler, sendWebSocketChatMessage } from "..";
import { useUIStore } from "../../../stores/modules/ui/ui";

export class ChatHandler {
  private messageQueue: WebSocketChatMessage[] = [];
  private currentMessagePart: WebSocketChatMessage | null = null;
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
      if (data.isMultiPart) {
        this.handleMultiPartMessage(data);
      } else {
        this.handleSingleMessage(data);
      }
      gameStore.currentStatus = "responding";
    } catch (error) {
      console.error("处理回复消息出错:", error);
      gameStore.currentStatus = "input";
    }
  }

  private handleMultiPartMessage(data: WebSocketChatMessage) {
    this.messageQueue.push(data);

    if (data.partIndex === 0 && !this.isProcessing) {
      this.continueMessage();
    }
  }

  private handleSingleMessage(data: WebSocketChatMessage) {
    const gameStore = useGameStore();

    const chatMessage = {
      content: data.message || data.content || "",
      emotion: data.emotion,
      audioFile: data.audioFile,
      isFinal: true,
      motionText: data.motionText,
      originalTag: data.originalTag,
    };

    gameStore.addToDialogHistory({
      type: "reply",
      ...chatMessage,
    });

    gameStore.currentLine = chatMessage.content;
    gameStore.avatar.emotion = chatMessage.emotion || "正常";
  }

  private processNextMessage() {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    if (this.messageQueue.length === 0) {
      this.isProcessing = false;
      return;
    }

    this.isProcessing = true;
    this.currentMessagePart = this.messageQueue.shift() || null;

    if (!this.currentMessagePart) return;

    const displayText = this.currentMessagePart.motionText
      ? `${this.currentMessagePart.message}（${this.currentMessagePart.motionText}）`
      : this.currentMessagePart.message || "";

    const isFinal =
      this.currentMessagePart.partIndex ===
      this.currentMessagePart.totalParts! - 1;

    this.currentMessagePart.isFinal = isFinal;

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

  public continueMessage() {
    if (this.currentMessagePart?.isFinal) {
      this.resetConversationState();
    } else {
      console.log(this.currentMessagePart?.isFinal);
      this.processNextMessage();
    }
  }

  private resetConversationState() {
    const gameStore = useGameStore();

    this.currentMessagePart = null;
    this.messageQueue = [];
    this.isProcessing = false;
    gameStore.currentStatus = "input";
    gameStore.currentLine = "";
  }
}

// 导出单例
export const chatHandler = new ChatHandler();

import type { ChatSocket } from "../../core/connection.js";
import EventBus from "../../core/event-bus.js";
import { HistoryController } from "../menu/history/controller.js";

interface MessageData {
  type: string;
  message?: string;
  content?: string;
  emotion?: string;
  audioFile?: string;
  isMultiPart?: boolean;
  partIndex?: number;
  totalParts?: number;
  motionText?: string;
  originalTag?: string;
  isFinal?: boolean;
}

interface ChatMessage {
  content: string;
  emotion?: string;
  audioFile?: string;
  isFinal: boolean;
  originalTag?: string;
  motionText?: string;
}

export class ChatManager {
  private connection: ChatSocket;
  private messageQueue: MessageData[];
  private currentMessagePart: MessageData | null;
  private isProcessing: boolean;
  private historyController: HistoryController;

  constructor({
    socket,
    historyController,
  }: {
    socket: ChatSocket;
    historyController: HistoryController;
  }) {
    this.connection = socket;
    this.historyController = historyController;
    this.messageQueue = [];
    this.currentMessagePart = null;
    this.isProcessing = false;
    this.setupSocketHandlers();
    this.setupEventListeners();
  }

  private setupSocketHandlers(): void {
    this.connection.onmessage((event: MessageEvent) => {
      try {
        const data: MessageData = JSON.parse(event.data);

        if (data.type === "reply") {
          if (data.isMultiPart) {
            this.handleMultiPartMessage(data);
          } else {
            this.handleSingleMessage(data);
          }
        }
      } catch (e) {
        console.error("消息解析错误:", e, "原始数据:", event.data);
        EventBus.emit("chat:error", { error: e, rawData: event.data });
      }
    });
  }

  private setupEventListeners(): void {
    EventBus.on("ui:send-message", (text: string) => {
      this.sendOrContinue(text);
    });

    EventBus.on("ui:continue", () => {
      this.handleContinue();
    });

    EventBus.on("chat:enable-input", () => {
      this.enableInput();
    });
  }

  private handleMultiPartMessage(data: MessageData): void {
    this.messageQueue.push(data);

    if (data.partIndex === 0 && !this.isProcessing) {
      this.processNextMessage();
    }
  }

  private handleSingleMessage(data: MessageData): void {
    console.log("【处理单条消息】", data);
    EventBus.emit("chat:message", {
      content: data.message || data.content || "",
      emotion: data.emotion,
      audioFile: data.audioFile,
      isFinal: true,
    } as ChatMessage);
  }

  private processNextMessage(): void {
    if (this.messageQueue.length === 0) {
      this.isProcessing = false;
      return;
    }

    this.isProcessing = true;
    this.currentMessagePart = this.messageQueue.shift() || null;

    let displayText = "";
    if (!this.currentMessagePart) {
      displayText = "";
    } else {
      displayText =
        this.currentMessagePart.motionText &&
        this.currentMessagePart.motionText !== ""
          ? `${this.currentMessagePart.message}（${this.currentMessagePart.motionText}）`
          : this.currentMessagePart.message || "";
    }

    if (this.currentMessagePart) {
      this.historyController.addMessage(
        null,
        displayText,
        this.currentMessagePart.partIndex ===
          this.currentMessagePart.totalParts! - 1
      );
    }

    EventBus.emit("chat:message", {
      content: this.currentMessagePart?.message || "",
      emotion: this.currentMessagePart?.emotion,
      originalTag: this.currentMessagePart?.originalTag,
      audioFile: this.currentMessagePart?.audioFile,
      isFinal:
        this.currentMessagePart?.partIndex ===
        this.currentMessagePart?.totalParts! - 1,
      motionText: this.currentMessagePart?.motionText,
    } as ChatMessage);

    console.log(this.currentMessagePart?.audioFile);
  }

  private sendOrContinue(text: string): void {
    if (this.isProcessing) {
      this.handleContinue();
    } else if (text) {
      this.sendMessage(text);
    }
  }

  private sendMessage(text: string): void {
    if (!text.trim()) return;

    const message = {
      type: "message",
      content: text,
    };

    EventBus.emit("chat:thinking", true);

    setTimeout(() => {
      this.connection.send(message);
    }, 600);
    this.historyController.addMessage(text, null, false);

    this.messageQueue = [];
    this.isProcessing = false;
  }

  private handleContinue(): void {
    if (
      this.currentMessagePart &&
      this.currentMessagePart.partIndex ===
        this.currentMessagePart.totalParts! - 1
    ) {
      this.resetConversationState();
      EventBus.emit("chat:conversation-end");
    } else {
      this.processNextMessage();
    }
  }

  private enableInput(): void {
    EventBus.emit("chat:input-enabled");
  }

  private resetConversationState(): void {
    this.currentMessagePart = null;
    this.messageQueue = [];
    this.isProcessing = false;

    EventBus.emit("chat:reset");
    EventBus.emit("chat:enable-input");
  }
}

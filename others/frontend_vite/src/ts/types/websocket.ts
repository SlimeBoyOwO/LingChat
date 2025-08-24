// WebSocket相关类型定义

export interface WebSocketMessage {
  type: string;
  data?: any;
}

export interface PingMessage extends WebSocketMessage {
  type: "ping";
}

export interface ChatMessage extends WebSocketMessage {
  type: "chat";
  data: {
    message: string;
    timestamp: number;
    sender?: string;
  };
}

export interface ConnectionEventListeners {
  message: ((event: MessageEvent) => void)[];
  open: (() => void)[];
  close: ((event: CloseEvent) => void)[];
  error: ((error: Event) => void)[];
}

export interface ChatSocketConfig {
  url: string;
  maxReconnects?: number;
  heartbeatInterval?: number;
}

export type WebSocketEventType = "message" | "open" | "close" | "error";

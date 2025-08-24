import EventBus from "./event-bus";
import type {
  ConnectionEventListeners,
  ChatSocketConfig,
  WebSocketEventType,
  WebSocketMessage,
} from "../types";
import { APP_CONFIG } from "./config";

export class ChatSocket {
  private url: string;
  private socket: WebSocket | null = null;
  private reconnectAttempts: number = 0;
  private maxReconnects: number;
  private heartbeatInterval: number;
  private heartbeatTimer: ReturnType<typeof setTimeout> | null = null;

  // 添加事件监听器容器
  private eventListeners: ConnectionEventListeners = {
    message: [],
    open: [],
    close: [],
    error: [],
  };

  constructor(url: string, config?: Partial<ChatSocketConfig>) {
    this.url = url;
    this.maxReconnects =
      config?.maxReconnects || APP_CONFIG.WEBSOCKET.MAX_RECONNECTS;
    this.heartbeatInterval =
      config?.heartbeatInterval || APP_CONFIG.WEBSOCKET.HEARTBEAT_INTERVAL;

    this.setupConnection();
  }

  private setupConnection(): void {
    this.socket = new WebSocket(this.url);

    // 转发原生WebSocket事件到自定义监听器
    this.socket.onmessage = (event: MessageEvent) => {
      this.eventListeners.message.forEach((cb) => cb(event));
    };

    this.socket.onopen = () => {
      this.reconnectAttempts = 0;
      this.startHeartbeat();
      this.eventListeners.open.forEach((cb) => cb());
      EventBus.emit("connection:open");
    };

    this.socket.onclose = (e: CloseEvent) => {
      this.eventListeners.close.forEach((cb) => cb(e));
      this.handleDisconnect(e);
    };

    this.socket.onerror = (err: Event) => {
      this.eventListeners.error.forEach((cb) => cb(err));
      EventBus.emit("connection:error", err);
    };
  }

  private startHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
    }

    this.heartbeatTimer = setInterval(() => {
      if (this.socket?.readyState === WebSocket.OPEN) {
        this.socket.send(JSON.stringify({ type: "ping" }));
      }
    }, this.heartbeatInterval);
  }

  private handleDisconnect(_event: CloseEvent): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }

    if (this.reconnectAttempts < this.maxReconnects) {
      const delay = Math.min(
        APP_CONFIG.WEBSOCKET.RECONNECT_DELAY_BASE *
          Math.pow(2, this.reconnectAttempts),
        APP_CONFIG.WEBSOCKET.MAX_RECONNECT_DELAY
      );
      this.reconnectAttempts++;

      setTimeout(() => {
        console.log(
          `尝试重新连接 (${this.reconnectAttempts}/${this.maxReconnects})`
        );
        this.setupConnection();
      }, delay);
    } else {
      EventBus.emit("connection:dead");
    }
  }

  /**
   * 发送消息
   * @param data 要发送的数据
   */
  send(data: WebSocketMessage | any): void {
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(data));
    } else {
      throw new Error("WebSocket未连接");
    }
  }

  /**
   * 关闭连接
   */
  close(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
    this.socket?.close();
  }

  /**
   * 获取连接状态
   */
  get readyState(): number {
    return this.socket?.readyState || WebSocket.CLOSED;
  }

  /**
   * 检查是否已连接
   */
  get isConnected(): boolean {
    return this.socket?.readyState === WebSocket.OPEN;
  }

  // 添加事件监听方法
  onmessage(callback: (event: MessageEvent) => void): void {
    this.eventListeners.message.push(callback);
  }

  onopen(callback: () => void): void {
    this.eventListeners.open.push(callback);
  }

  onclose(callback: (event: CloseEvent) => void): void {
    this.eventListeners.close.push(callback);
  }

  onerror(callback: (error: Event) => void): void {
    this.eventListeners.error.push(callback);
  }

  /**
   * 移除事件监听器
   */
  removeEventListener(type: WebSocketEventType, callback: Function): void {
    const listeners = this.eventListeners[type];
    const index = listeners.indexOf(callback as any);
    if (index > -1) {
      listeners.splice(index, 1);
    }
  }
}

import EventBus from "./event-bus.js";
export class ChatSocket {
  constructor(url) {
    this.url = url;
    this.socket = null;
    this.reconnectAttempts = 0;
    this.maxReconnects = 5;
    this.heartbeatInterval = 30000;

    // 添加事件监听器容器
    this.eventListeners = {
      message: [],
      open: [],
      close: [],
      error: [],
    };

    this.setupConnection();
  }

  setupConnection() {
    this.socket = new WebSocket(this.url);

    // 转发原生WebSocket事件到自定义监听器
    this.socket.onmessage = (event) => {
      this.eventListeners.message.forEach((cb) => cb(event));
    };

    this.socket.onopen = () => {
      this.reconnectAttempts = 0;
      this.startHeartbeat();
      this.eventListeners.open.forEach((cb) => cb());
      EventBus.emit("connection:open");
    };

    this.socket.onclose = (e) => {
      this.eventListeners.close.forEach((cb) => cb(e));
      this.handleDisconnect(e);
    };

    this.socket.onerror = (err) => {
      this.eventListeners.error.forEach((cb) => cb(err));
      EventBus.emit("connection:error", err);
    };
  }

  startHeartbeat() {
    this.heartbeatTimer = setInterval(() => {
      if (this.socket.readyState === WebSocket.OPEN) {
        this.socket.send(JSON.stringify({ type: "ping" }));
      }
    }, this.heartbeatInterval);
  }

  handleDisconnect(event) {
    clearInterval(this.heartbeatTimer);

    if (this.reconnectAttempts < this.maxReconnects) {
      const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
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

  send(data) {
    if (this.socket.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(data));
    } else {
      throw new Error("WebSocket未连接");
    }
  }

  close() {
    clearInterval(this.heartbeatTimer);
    this.socket.close();
  }

  // 添加事件监听方法
  onmessage(callback) {
    this.eventListeners.message.push(callback);
  }

  onopen(callback) {
    this.eventListeners.open.push(callback);
  }

  onclose(callback) {
    this.eventListeners.close.push(callback);
  }

  onerror(callback) {
    this.eventListeners.error.push(callback);
  }
}

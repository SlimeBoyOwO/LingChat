import { ref } from "vue";
import type { WebSocketHandler } from "../../types";

const socket = ref<WebSocket | null>(null);
const handlers = new Map<string, WebSocketHandler>();
const reconnectAttempts = ref(0);
const maxReconnectAttempts = 5;
const reconnectDelay = 3000;

export const connectWebSocket = (url: string) => {
  socket.value = new WebSocket(url);

  socket.value.onopen = () => {
    const timestamp = new Date().toISOString();
    console.log(`[${timestamp}] WebSocket连接已建立，URL: ${url}`);
    reconnectAttempts.value = 0;
  };

  socket.value.onmessage = (event) => {
    // 增加详细日志
    const timestamp = new Date().toISOString();
    console.log(`[${timestamp}] WebSocket收到消息:`, event.data);
    
    try {
      const message = JSON.parse(event.data);
      console.log(`[${timestamp}] 解析后的消息对象:`, message);
      
      const handler = handlers.get(message.type);
      if (handler) {
        console.log(`[${timestamp}] 找到处理器，处理消息类型: ${message.type}`);
        handler(message);
      } else {
        console.warn(`[${timestamp}] 没有找到消息类型 ${message.type} 的处理器`);
      }
    } catch (error) {
      console.error(`[${timestamp}] 解析WebSocket消息错误:`, error);
    }
  };

  socket.value.onclose = (event) => {
    const timestamp = new Date().toISOString();
    console.log(`[${timestamp}] WebSocket连接已关闭，代码: ${event.code}, 原因: ${event.reason}, 是否干净关闭: ${event.wasClean}`);
    
    if (reconnectAttempts.value < maxReconnectAttempts) {
      console.log(`[${timestamp}] 尝试重新连接 (${reconnectAttempts.value + 1}/${maxReconnectAttempts})，${reconnectDelay}ms后重试`);
      setTimeout(() => {
        reconnectAttempts.value++;
        connectWebSocket(url);
      }, reconnectDelay);
    } else {
      console.error(`[${timestamp}] 已达到最大重连次数，停止重连`);
    }
  };

  socket.value.onerror = (error) => {
    const timestamp = new Date().toISOString();
    console.error(`[${timestamp}] WebSocket错误:`, error);
    console.error(`[${timestamp}] WebSocket当前状态: ${socket.value?.readyState}`);
  };
};

export const registerHandler = (type: string, handler: WebSocketHandler) => {
  handlers.set(type, handler);
};

export const unregisterHandler = (type: string) => {
  handlers.delete(type);
};

export const sendWebSocketMessage = (type: string, data: any) => {
  const timestamp = new Date().toISOString();
  const message = JSON.stringify({ type, data });
  
  if (socket.value?.readyState === WebSocket.OPEN) {
    console.log(`[${timestamp}] 发送WebSocket消息:`, message);
    socket.value.send(message);
    return true;
  } else {
    console.warn(`[${timestamp}] 无法发送消息，WebSocket未连接。当前状态: ${socket.value?.readyState}`);
    return false;
  }
};

export const sendWebSocketChatMessage = (type: string, content: string) => {
  const timestamp = new Date().toISOString();
  const message = JSON.stringify({ type, content });
  
  if (socket.value?.readyState === WebSocket.OPEN) {
    console.log(`[${timestamp}] 发送WebSocket聊天消息:`, message);
    socket.value.send(message);
    return true;
  } else {
    console.warn(`[${timestamp}] 无法发送聊天消息，WebSocket未连接。当前状态: ${socket.value?.readyState}`);
    return false;
  }
};

export const closeWebSocket = () => {
  if (socket.value) {
    socket.value.close();
    socket.value = null;
  }
};

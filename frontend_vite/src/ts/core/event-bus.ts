import type { EventCallback, EventListeners } from "../types";

class EventBus {
  private listeners: EventListeners = {};

  /**
   * 注册事件监听器
   * @param event 事件名称
   * @param callback 回调函数
   */
  on<T = any>(event: string, callback: EventCallback<T>): void {
    if (!this.listeners[event]) {
      this.listeners[event] = [];
    }
    this.listeners[event].push(callback);
  }

  /**
   * 移除事件监听器
   * @param event 事件名称
   * @param callback 要移除的回调函数
   */
  off<T = any>(event: string, callback: EventCallback<T>): void {
    if (!this.listeners[event]) return;

    this.listeners[event] = this.listeners[event].filter(
      (listener) => listener !== callback
    );
  }

  /**
   * 触发事件
   * @param event 事件名称
   * @param data 事件数据
   */
  emit<T = any>(event: string, data?: T): void {
    if (this.listeners[event]) {
      this.listeners[event].forEach((callback) => {
        try {
          callback(data);
        } catch (e) {
          console.error(`事件处理错误 [${event}]:`, e);
        }
      });
    }
  }

  /**
   * 移除所有事件监听器
   * @param event 可选，指定事件名称，不指定则清除所有
   */
  removeAllListeners(event?: string): void {
    if (event) {
      delete this.listeners[event];
    } else {
      this.listeners = {};
    }
  }

  /**
   * 获取指定事件的监听器数量
   * @param event 事件名称
   * @returns 监听器数量
   */
  listenerCount(event: string): number {
    return this.listeners[event]?.length || 0;
  }
}

const eventBusInstance = new EventBus();
export default eventBusInstance;

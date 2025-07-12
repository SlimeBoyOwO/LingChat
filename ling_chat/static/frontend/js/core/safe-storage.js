/**
 * 安全的本地存储操作工具
 * 自动处理JSON序列化和错误捕获
 */
export class SafeStorage {
  constructor(storage = localStorage) {
    this.storage = storage;
  }

  /**
   * 安全获取存储数据
   * @param {string} key
   * @param {any} defaultValue
   * @returns {any}
   */
  get(key, defaultValue = null) {
    try {
      const value = this.storage.getItem(key);
      return value ? JSON.parse(value) : defaultValue;
    } catch (error) {
      console.error(`读取存储失败 [${key}]:`, error);
      return defaultValue;
    }
  }

  /**
   * 安全设置存储数据
   * @param {string} key
   * @param {any} value
   */
  set(key, value) {
    try {
      this.storage.setItem(key, JSON.stringify(value));
    } catch (error) {
      console.error(`存储数据失败 [${key}]:`, error);

      // 存储空间不足时自动清理过期的项目
      if (error.name === "QuotaExceededError") {
        this._clearExpiredItems();
        this.set(key, value); // 重试一次
      }
    }
  }

  /**
   * 移除指定键值
   * @param {string} key
   */
  remove(key) {
    this.storage.removeItem(key);
  }

  /**
   * 清空所有存储
   */
  clear() {
    this.storage.clear();
  }

  /**
   * 清理过期项目（私有方法）
   */
  _clearExpiredItems() {
    const now = Date.now();
    Object.keys(this.storage).forEach((key) => {
      try {
        const data = JSON.parse(this.storage.getItem(key));
        if (data?.expiresAt && data.expiresAt < now) {
          this.storage.removeItem(key);
        }
      } catch {
        // 非JSON数据直接跳过
      }
    });
  }

  /**
   * 设置带过期时间的数据
   * @param {string} key
   * @param {any} value
   * @param {number} ttl 有效期（毫秒）
   */
  setWithTTL(key, value, ttl) {
    this.set(key, {
      value,
      expiresAt: Date.now() + ttl,
    });
  }
}

// 默认导出实例（使用localStorage）
export const safeStorage = new SafeStorage();

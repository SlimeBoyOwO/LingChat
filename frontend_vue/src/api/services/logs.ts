/**
 * 前端控制台日志转发服务
 * 将前端控制台输出转发到后端进行统一日志管理
 */
import http from "../http";

// 日志级别枚举
export enum ConsoleLogLevel {
  LOG = "log",
  INFO = "info",
  WARN = "warn",
  ERROR = "error",
  DEBUG = "debug",
  TRACE = "trace",
}

// 日志来源枚举
export enum ConsoleLogSource {
  FRONTEND = "frontend",
  BACKEND = "backend",
  SYSTEM = "system",
  NETWORK = "network",
  DATABASE = "database",
  AI_SERVICE = "ai_service",
  UNKNOWN = "unknown",
}

// 日志条目接口
export interface ConsoleLogEntry {
  timestamp?: string;
  level: ConsoleLogLevel;
  message: string;
  source?: ConsoleLogSource;
  context?: Record<string, any>;
  stack_trace?: string;
  component?: string;
  url?: string;
  line_number?: number;
  column_number?: number;
  session_id?: string;
  user_id?: number;
  request_id?: string;
  metadata?: Record<string, any>;
}

// 批量日志接口
export interface ConsoleLogBatch {
  logs: ConsoleLogEntry[];
  batch_id?: string;
  session_id?: string;
  user_id?: number;
}

// 日志转发配置
export interface LogForwardConfig {
  enabled: boolean; // 是否启用日志转发
  minLevel: ConsoleLogLevel; // 最小转发级别
  batchSize: number; // 批量发送大小
  flushInterval: number; // 批量发送间隔（毫秒）
  maxRetries: number; // 最大重试次数
  excludePatterns: string[]; // 排除模式
}

// 默认配置（实时模式）
const DEFAULT_CONFIG: LogForwardConfig = {
  enabled: true,
  minLevel: ConsoleLogLevel.TRACE, // 转发所有级别日志，由后端过滤
  batchSize: 10, // 保留但不再使用批量大小检查
  flushInterval: 0, // 禁用定时器，使用防抖实时发送
  maxRetries: 3,
  excludePatterns: [],
};

/**
 * 控制台日志转发服务
 */
class ConsoleLogForwarder {
  private config: LogForwardConfig;
  private logQueue: ConsoleLogEntry[] = [];
  private flushTimer: number | null = null;
  private isFlushing = false;
  private sessionId: string;
  private userId: number | null = null;
  private pendingFlush: number | null = null; // 防抖定时器ID

  constructor(config?: Partial<LogForwardConfig>) {
    this.config = { ...DEFAULT_CONFIG, ...config };
    this.sessionId = this.generateSessionId();
    this.initialize();
  }

  /**
   * 初始化日志转发器
   */
  private initialize(): void {
    if (!this.config.enabled) {
      console.log("[LogForwarder] 日志转发已禁用");
      return;
    }

    // 记录配置信息
    console.log(`[LogForwarder] 初始化配置: enabled=${this.config.enabled}, minLevel=${this.config.minLevel}`);

    // 重写控制台方法
    this.overrideConsoleMethods();

    // 监听页面卸载事件
    window.addEventListener("beforeunload", () => this.flushImmediately());

    console.log("[LogForwarder] 日志转发器已初始化（实时模式）");
  }

  /**
   * 生成会话ID
   */
  private generateSessionId(): string {
    return `session_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }

  /**
   * 重写控制台方法
   */
  private overrideConsoleMethods(): void {
    const originalConsole = {
      log: console.log,
      info: console.info,
      warn: console.warn,
      error: console.error,
      debug: console.debug,
      trace: console.trace,
    };

    // 重写console.log
    console.log = (...args: any[]) => {
      this.captureLog(ConsoleLogLevel.LOG, args);
      originalConsole.log.apply(console, args);
    };

    // 重写console.info
    console.info = (...args: any[]) => {
      this.captureLog(ConsoleLogLevel.INFO, args);
      originalConsole.info.apply(console, args);
    };

    // 重写console.warn
    console.warn = (...args: any[]) => {
      this.captureLog(ConsoleLogLevel.WARN, args);
      originalConsole.warn.apply(console, args);
    };

    // 重写console.error
    console.error = (...args: any[]) => {
      this.captureLog(ConsoleLogLevel.ERROR, args);
      originalConsole.error.apply(console, args);
    };

    // 重写console.debug
    console.debug = (...args: any[]) => {
      this.captureLog(ConsoleLogLevel.DEBUG, args);
      originalConsole.debug.apply(console, args);
    };

    // 重写console.trace
    console.trace = (...args: any[]) => {
      this.captureLog(ConsoleLogLevel.TRACE, args);
      originalConsole.trace.apply(console, args);
    };
  }

  /**
   * 捕获日志
   */
  private captureLog(level: ConsoleLogLevel, args: any[]): void {
    if (!this.config.enabled) return;

    // 检查日志级别
    if (this.shouldSkipLevel(level)) return;

    try {
      // 解析日志消息
      const message = this.parseLogMessage(args);

      // 检查排除模式
      if (this.shouldExcludeMessage(message)) return;

      // 创建日志条目
      const logEntry: ConsoleLogEntry = {
        timestamp: new Date().toISOString(),
        level,
        message,
        source: ConsoleLogSource.FRONTEND,
        component: this.getComponentName(),
        url: window.location.href,
        session_id: this.sessionId,
        user_id: this.userId || undefined,
        metadata: {
          userAgent: navigator.userAgent,
          platform: navigator.platform,
        },
      };

      // 添加堆栈跟踪（对于错误和警告）
      if (level === ConsoleLogLevel.ERROR || level === ConsoleLogLevel.WARN) {
        logEntry.stack_trace = this.getStackTrace();
      }

      // 添加到队列
      this.addToQueue(logEntry);

    } catch (error) {
      // 避免日志捕获本身产生无限循环
      console.error("[LogForwarder] 捕获日志失败:", error);
    }
  }

  /**
   * 检查是否应该跳过此级别
   */
  private shouldSkipLevel(level: ConsoleLogLevel): boolean {
    const levelOrder = {
      [ConsoleLogLevel.TRACE]: 0,
      [ConsoleLogLevel.DEBUG]: 1,
      [ConsoleLogLevel.LOG]: 2,
      [ConsoleLogLevel.INFO]: 3,
      [ConsoleLogLevel.WARN]: 4,
      [ConsoleLogLevel.ERROR]: 5,
    };

    const currentLevel = levelOrder[level] !== undefined ? levelOrder[level] : 2;
    const minLevel = levelOrder[this.config.minLevel] !== undefined ? levelOrder[this.config.minLevel] : 3;

    // 调试日志（已注释，过于频繁）
    // if (level === ConsoleLogLevel.LOG) {
    //   console.debug(`[LogForwarder] 检查LOG级别: currentLevel=${currentLevel}, minLevel=${minLevel}, config.minLevel=${this.config.minLevel}, shouldSkip=${currentLevel < minLevel}`);
    // }

    return currentLevel < minLevel;
  }

  /**
   * 解析日志消息
   */
  private parseLogMessage(args: any[]): string {
    try {
      return args
        .map(arg => {
          if (typeof arg === "object") {
            try {
              return JSON.stringify(arg);
            } catch {
              return String(arg);
            }
          }
          return String(arg);
        })
        .join(" ");
    } catch {
      return "无法解析日志消息";
    }
  }

  /**
   * 检查是否应该排除此消息
   */
  private shouldExcludeMessage(message: string): boolean {
    if (!this.config.excludePatterns.length) return false;

    const lowerMessage = message.toLowerCase();
    return this.config.excludePatterns.some(pattern =>
      lowerMessage.includes(pattern.toLowerCase())
    );
  }

  /**
   * 获取组件名称
   */
  private getComponentName(): string {
    try {
      // 尝试从Vue组件获取名称
      const vueInstance = (window as any).__VUE_DEVTOOLS_GLOBAL_HOOK__?.Vue?.prototype;
      if (vueInstance && vueInstance.$options && vueInstance.$options.name) {
        return vueInstance.$options.name;
      }
    } catch {
      // 忽略错误
    }
    return "Unknown";
  }

  /**
   * 获取堆栈跟踪
   */
  private getStackTrace(): string {
    try {
      const error = new Error();
      return error.stack || "";
    } catch {
      return "";
    }
  }

  /**
   * 添加到队列并立即触发发送（防抖）
   */
  private addToQueue(logEntry: ConsoleLogEntry): void {
    this.logQueue.push(logEntry);

    // 取消之前的防抖定时器
    if (this.pendingFlush) {
      clearTimeout(this.pendingFlush);
    }

    // 设置新的防抖定时器，50ms后发送
    // 这样既能实时发送，又能合并短时间内的大量日志
    this.pendingFlush = window.setTimeout(() => {
      this.pendingFlush = null;
      this.flush();
    }, 50);
  }

  /**
   * 设置批量发送定时器
   */
  private setupFlushTimer(): void {
    if (this.flushTimer) {
      clearInterval(this.flushTimer);
    }

    this.flushTimer = window.setInterval(() => {
      if (this.logQueue.length > 0) {
        this.flush();
      }
    }, this.config.flushInterval);
  }

  /**
   * 批量发送日志
   */
  private async flush(): Promise<void> {
    if (this.isFlushing || this.logQueue.length === 0) return;

    this.isFlushing = true;
    let logsToSend: ConsoleLogEntry[] = [];

    try {
      logsToSend = [...this.logQueue];
      this.logQueue = [];

      const logBatch: ConsoleLogBatch = {
        logs: logsToSend,
        batch_id: `batch_${Date.now()}`,
        session_id: this.sessionId,
        user_id: this.userId || undefined,
      };

      // 发送到后端
      await this.sendLogs(logBatch);

      // 调试日志（已注释，过于频繁）
      // console.debug(`[LogForwarder] 已发送 ${logsToSend.length} 条日志`);

    } catch (error) {
      console.error("[LogForwarder] 发送日志失败:", error);
      // 发送失败，将日志重新放回队列（保留最近的部分）
      this.logQueue = [...this.logQueue, ...logsToSend].slice(-this.config.batchSize * 2);
    } finally {
      this.isFlushing = false;
    }
  }

  /**
   * 立即发送所有日志
   */
  public async flushImmediately(): Promise<void> {
    if (this.logQueue.length === 0) return;

    console.log(`[LogForwarder] 立即发送 ${this.logQueue.length} 条日志`);
    await this.flush();
  }

  /**
   * 发送日志到后端
   */
  private async sendLogs(logBatch: ConsoleLogBatch): Promise<void> {
    try {
      // 后端API期望 { "log_batch": ... } 格式
      await http.post("/v1/logs/console/batch", { log_batch: logBatch });
    } catch (error: any) {
      // 如果后端不支持日志API，静默失败
      if (error.status === 404) {
        console.warn("[LogForwarder] 后端日志API未启用，禁用日志转发");
        this.config.enabled = false;
      } else {
        throw error;
      }
    }
  }

  /**
   * 手动记录日志
   */
  public log(entry: ConsoleLogEntry): void {
    if (!this.config.enabled) return;

    const fullEntry: ConsoleLogEntry = {
      timestamp: new Date().toISOString(),
      session_id: this.sessionId,
      user_id: this.userId || undefined,
      source: ConsoleLogSource.FRONTEND,
      ...entry,
    };

    this.addToQueue(fullEntry);
  }

  /**
   * 更新配置
   */
  public updateConfig(config: Partial<LogForwardConfig>): void {
    this.config = { ...this.config, ...config };

    // 重新设置定时器
    if (config.flushInterval !== undefined) {
      this.setupFlushTimer();
    }
  }

  /**
   * 设置用户ID
   */
  public setUserId(userId: number): void {
    this.userId = userId;
  }

  /**
   * 获取当前配置
   */
  public getConfig(): LogForwardConfig {
    return { ...this.config };
  }

  /**
   * 获取队列状态
   */
  public getQueueStatus(): { length: number; isFlushing: boolean } {
    return {
      length: this.logQueue.length,
      isFlushing: this.isFlushing,
    };
  }

  /**
   * 销毁日志转发器
   */
  public destroy(): void {
    if (this.flushTimer) {
      clearInterval(this.flushTimer);
      this.flushTimer = null;
    }

    // 发送剩余日志
    this.flushImmediately().catch(() => {
      // 忽略错误
    });

    console.log("[LogForwarder] 日志转发器已销毁");
  }
}

// 创建全局实例
const logForwarder = new ConsoleLogForwarder();

// 导出实例和类型
export { logForwarder };
export default logForwarder;
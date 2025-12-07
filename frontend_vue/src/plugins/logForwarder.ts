/**
 * Vue日志转发插件
 * 提供Vue组件错误捕获和日志集成
 */
import { App, Plugin } from "vue";
import logForwarder, { ConsoleLogLevel, ConsoleLogSource } from "../api/services/logs";

// Vue错误处理器
const vueErrorHandler = (error: unknown, vm: any, info: string) => {
  // 记录Vue组件错误
  const errorMessage = error instanceof Error ? error.message : String(error);
  const stackTrace = error instanceof Error ? error.stack : undefined;

  logForwarder.log({
    level: ConsoleLogLevel.ERROR,
    message: `Vue组件错误: ${errorMessage}`,
    source: ConsoleLogSource.FRONTEND,
    component: vm?.$options?.name || "UnknownComponent",
    stack_trace: stackTrace,
    context: {
      vueInfo: info,
      componentName: vm?.$options?.name,
      componentPath: vm?.$el?.outerHTML?.substring(0, 200),
    },
  });
};

// 全局错误处理器
const globalErrorHandler = (event: ErrorEvent) => {
  logForwarder.log({
    level: ConsoleLogLevel.ERROR,
    message: `全局错误: ${event.message}`,
    source: ConsoleLogSource.FRONTEND,
    url: event.filename,
    line_number: event.lineno,
    column_number: event.colno,
    stack_trace: event.error?.stack,
  });
};

// Promise错误处理器
const unhandledRejectionHandler = (event: PromiseRejectionEvent) => {
  logForwarder.log({
    level: ConsoleLogLevel.ERROR,
    message: `未处理的Promise拒绝: ${event.reason?.message || event.reason}`,
    source: ConsoleLogSource.FRONTEND,
    context: {
      reason: event.reason,
    },
  });
};

// 网络错误处理器
const networkErrorHandler = (event: Event) => {
  const target = event.target as any;

  if (target.tagName === "IMG") {
    logForwarder.log({
      level: ConsoleLogLevel.WARN,
      message: `图片加载失败: ${target.src}`,
      source: ConsoleLogSource.NETWORK,
      component: "ImageLoader",
      url: target.src,
    });
  } else if (target.tagName === "SCRIPT") {
    logForwarder.log({
      level: ConsoleLogLevel.ERROR,
      message: `脚本加载失败: ${target.src}`,
      source: ConsoleLogSource.NETWORK,
      component: "ScriptLoader",
      url: target.src,
    });
  } else if (target.tagName === "LINK") {
    logForwarder.log({
      level: ConsoleLogLevel.WARN,
      message: `样式表加载失败: ${target.href}`,
      source: ConsoleLogSource.NETWORK,
      component: "StyleLoader",
      url: target.href,
    });
  }
};

// 性能监控
const performanceMonitor = () => {
  if ("performance" in window) {
    const perfEntries = performance.getEntriesByType("navigation");
    if (perfEntries.length > 0) {
      const navEntry = perfEntries[0] as PerformanceNavigationTiming;

      logForwarder.log({
        level: ConsoleLogLevel.INFO,
        message: "页面性能指标",
        source: ConsoleLogSource.SYSTEM,
        context: {
          dnsTime: navEntry.domainLookupEnd - navEntry.domainLookupStart,
          tcpTime: navEntry.connectEnd - navEntry.connectStart,
          requestTime: navEntry.responseEnd - navEntry.requestStart,
          domContentLoaded: navEntry.domContentLoadedEventEnd - navEntry.domContentLoadedEventStart,
          loadTime: navEntry.loadEventEnd - navEntry.loadEventStart,
          totalTime: navEntry.loadEventEnd - navEntry.startTime,
        },
      });
    }
  }
};

// Vue指令：日志记录
const vLog = {
  mounted(el: HTMLElement, binding: any) {
    const { value } = binding;
    const eventName = value?.event || "click";
    const message = value?.message || `元素 ${el.tagName} 被点击`;
    const level = value?.level || ConsoleLogLevel.INFO;

    el.addEventListener(eventName, () => {
      logForwarder.log({
        level,
        message,
        source: ConsoleLogSource.FRONTEND,
        component: "VLogDirective",
        context: {
          element: el.tagName,
          event: eventName,
          id: el.id,
          className: el.className,
        },
      });
    });
  },
};

// 插件安装函数
const install: Plugin = {
  install(app: App, options?: any) {
    // 注册Vue错误处理器
    app.config.errorHandler = vueErrorHandler;

    // 注册全局错误处理器
    window.addEventListener("error", globalErrorHandler);
    window.addEventListener("unhandledrejection", unhandledRejectionHandler);

    // 注册网络错误处理器
    window.addEventListener("error", networkErrorHandler, true);

    // 性能监控（页面加载完成后）
    if (document.readyState === "complete") {
      performanceMonitor();
    } else {
      window.addEventListener("load", performanceMonitor);
    }

    // 注册自定义指令
    app.directive("log", vLog);

    // 将日志转发器添加到Vue原型，方便组件内访问
    app.config.globalProperties.$log = logForwarder;

    // 将日志转发器添加到Vue实例属性
    app.provide("logForwarder", logForwarder);

    // 记录插件初始化
    logForwarder.log({
      level: ConsoleLogLevel.INFO,
      message: "Vue日志转发插件已初始化",
      source: ConsoleLogSource.FRONTEND,
      component: "LogForwarderPlugin",
      context: options,
    });

    console.log("[LogForwarderPlugin] Vue日志转发插件已安装");
  },
};

// 组合式API支持
export const useLogForwarder = () => {
  return logForwarder;
};

// 导出插件
export default install;

// 导出类型和工具函数
export { ConsoleLogLevel, ConsoleLogSource };
export type { ConsoleLogEntry } from "../api/services/logs";
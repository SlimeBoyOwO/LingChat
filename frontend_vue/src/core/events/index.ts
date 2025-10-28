import { eventProcessorManager } from "./event-processor";
import type { IEventProcessor } from "./event-processor";

// 动态导入所有处理器
export async function initializeEventProcessors() {
  // 使用 import.meta.glob 动态导入 processors 目录下的所有 ts 文件
  const processorModules = import.meta.glob("./processors/*.ts", {
    eager: true,
  });

  for (const path in processorModules) {
    const module = processorModules[path] as any;

    // 尝试获取默认导出
    const ProcessorClass = module.default;

    if (ProcessorClass && typeof ProcessorClass === "function") {
      try {
        const processorInstance = new ProcessorClass() as IEventProcessor;
        eventProcessorManager.registerProcessor(processorInstance);
        console.log(`✅ Registered processor from: ${path}`);
      } catch (error) {
        console.error(
          `❌ Failed to instantiate processor from ${path}:`,
          error
        );
      }
    } else {
      console.warn(`⚠️ Module at ${path} does not export a default class`);
    }
  }
}

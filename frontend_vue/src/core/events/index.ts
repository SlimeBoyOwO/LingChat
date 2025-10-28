import { eventProcessorManager } from "./event-processor";
import { NarrationProcessor } from "./processors/narration-processor";
import { DialogueProcessor } from "./processors/dialogue-processor";
import { BackgroundProcessor } from "./processors/background-processor";

// 注册所有处理器
export function initializeEventProcessors() {
  eventProcessorManager.registerProcessor(new NarrationProcessor());
  eventProcessorManager.registerProcessor(new DialogueProcessor());
  eventProcessorManager.registerProcessor(new BackgroundProcessor());
}

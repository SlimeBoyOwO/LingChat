import type { ScriptEventType } from "../../types";

export interface IEventProcessor {
  processEvent(event: ScriptEventType, signal?: AbortSignal): Promise<void>;
  canHandle(eventType: string): boolean;
}

export class EventProcessorManager {
  private processors: IEventProcessor[] = [];

  registerProcessor(processor: IEventProcessor) {
    this.processors.push(processor);
  }

  async processEvent(event: ScriptEventType, signal?: AbortSignal): Promise<boolean> {
    const processor = this.processors.find((p) => p.canHandle(event.type));
    if (processor) {
      await processor.processEvent(event, signal);
      return true;
    }
    return false;
  }
}

export const eventProcessorManager = new EventProcessorManager();

import BackgroundEffectProcessor from "./processors/background-effect-processor";
import type { IEventProcessor } from "@lingchat/host/core/events/event-processor";
import ConsoleWindowProcessor from "./processors/console-window-processor";
import ForceChoiceProcessor from "./processors/force-choice-processor";
import GlitchWindowProcessor from "./processors/glitch-window-processor";
import JumpscareProcessor from "./processors/jumpscare-processor";
import PoemGameProcessor from "./processors/poem-game-processor";
import VoiceShiftProcessor from "./processors/voice-shift-processor";
import WindowTitleProcessor from "./processors/window-title-processor";

/** Explicit entry point: the host's local import.meta.glob cannot scan a package. */
export function createStoryEventProcessors(): IEventProcessor[] {
  return [
    new BackgroundEffectProcessor(),
    new ConsoleWindowProcessor(),
    new ForceChoiceProcessor(),
    new GlitchWindowProcessor(),
    new JumpscareProcessor(),
    new PoemGameProcessor(),
    new VoiceShiftProcessor(),
    new WindowTitleProcessor(),
    // The shared FIFO owns the authored wait duration.
    { canHandle: (type) => type === "wait", processEvent: async () => {} },
  ];
}

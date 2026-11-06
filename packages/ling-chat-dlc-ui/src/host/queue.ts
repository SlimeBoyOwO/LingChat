import type { ScriptEventType } from "@lingchat/host/types/script";
import { useUIStore } from "@lingchat/host/stores/modules/ui/ui";
export const resetExtensionQueue = () => useUIStore().resetHorrorEffects();
export const isSelfTimedVisual = (event: ScriptEventType) =>
  event.type === "modify_character" && event.flash === true;

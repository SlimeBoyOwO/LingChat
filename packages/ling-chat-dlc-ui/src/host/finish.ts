import { invoke } from "@tauri-apps/api/core";
import { eventQueue } from "@lingchat/host/core/events/event-queue";
import { useUIStore } from "@lingchat/host/stores/modules/ui/ui";
import type { ScriptInfo } from "@lingchat/host/stores/modules/game/state";
import router from "@lingchat/host/router";
import { resetScriptWindowTitle } from "../utils/windowTitleCoordinator";
import { storyPolicy } from "./policy";
export async function finishExtensionStory(script: ScriptInfo | null) {
  // Natural backend completion can happen while visual events are still queued.
  // Close native glitch windows only now, after every preceding beat was shown.
  try {
    await invoke("close_script_glitch_windows");
  } catch (error) {
    console.error("[ScriptEndProcessor] failed to close glitch windows:", error);
  }

  // Cancel this run's queue epoch as part of the same teardown, so a late
  // timer/reply cannot resume behind the main menu or a new story run.
  eventQueue.clear();

  const { returnToMenu } = storyPolicy(script);
  return async () => {
    useUIStore().resetHorrorEffects();
    resetScriptWindowTitle();
    if (returnToMenu && router.currentRoute.value.path === "/chat") await router.replace("/");
  };
}

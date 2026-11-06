import type { ScriptInfo } from "@lingchat/host/stores/modules/game/state";
export const storyPolicy = (script: ScriptInfo | null) => ({
  lockPet: script?.contentWarning === "horror",
  petHint: script?.contentWarning === "horror" ? "锁死了。从打开的那一刻起就锁死了。" : "",
  returnToMenu: script?.contentWarning === "horror",
});

/** Return true after an extension has handled the close presentation. Saving remains host-owned. */
export async function beforeExtensionClose(): Promise<boolean> {
  const { useUIStore } = await import("@lingchat/host/stores/modules/ui/ui");
  const ui = useUIStore();
  if (!ui.ghostLock) return false;
  ui.triggerGhostQuitZoom();
  await new Promise<void>((resolve) => window.setTimeout(resolve, 620));
  return true;
}

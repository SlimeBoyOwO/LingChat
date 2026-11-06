import { useUIStore } from "@lingchat/host/stores/modules/ui/ui";
import { isOwnedByStandaloneDlc } from "../utils/dlcMediaOwnership";
import { listen } from "@tauri-apps/api/event";
import { eventQueue } from "@lingchat/host/core/events/event-queue";
import type { ScriptEventType } from "@lingchat/host/types";
import { useGameStore } from "@lingchat/host/stores/modules/game";
import { resetScriptWindowTitle } from "../utils/windowTitleCoordinator";
type EventFactory = (
  payload: unknown,
  defaults: { type: string; defaultDuration: number; isFinal?: boolean },
) => ScriptEventType;
export function registerExtensionListeners(asEvent: EventFactory) {
  listen<string>("script:prepare-uninstall", (event) => {
    const folderKey = event.payload;
    console.log("[Tauri] script:prepare-uninstall", folderKey);
    const gameStore = useGameStore();
    const uiStore = useUIStore();
    const targetIsActive = gameStore.runningScript?.folderKey === folderKey;

    // 只有目标 DLC 自己仍占用剧情队列时才结束剧情；卸载无关 DLC 绝不能
    // 中断自由对话或另一个剧本。
    if (targetIsActive) {
      eventQueue.clear();
      gameStore.forceChoice = null;
      gameStore.poemGame = null;
      // 后端已持有 DLC 生命周期 reservation，只做前端释放，不再 invoke stop_script。
      gameStore.exitStoryMode(false);
      uiStore.resetHorrorEffects();
      resetScriptWindowTitle();
    }

    if (isOwnedByStandaloneDlc(uiStore.currentBackgroundMusic, folderKey)) {
      uiStore.currentBackgroundMusic = "None";
    }
    if (isOwnedByStandaloneDlc(uiStore.currentPresentPic, folderKey)) {
      uiStore.currentPresentPic = "";
      uiStore.currentPresentPicScale = 1;
    }
    if (isOwnedByStandaloneDlc(uiStore.currentSoundEffect, folderKey)) {
      uiStore.triggerSoundEffect("None");
    }
    uiStore.ambientTracks = uiStore.ambientTracks.filter(
      (track) => !isOwnedByStandaloneDlc(track.src, folderKey),
    );

    window.dispatchEvent(new CustomEvent("lingchat:release-dlc-media", { detail: { folderKey } }));
  });

  listen("script:voice-shift", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "voice_shift", defaultDuration: 0 }));
  });
  listen("script:jumpscare", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "jumpscare", defaultDuration: 0.6 }));
  });
  listen("script:wait", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "wait", defaultDuration: 0 }));
  });
  listen("script:window-title", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "window_title", defaultDuration: 0 }));
  });
  listen("script:window-title-reset", () => {
    resetScriptWindowTitle();
  });
  listen("script:glitch-window", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "glitch_window", defaultDuration: 0 }));
  });
  listen("script:console-window", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "console_window", defaultDuration: 0 }));
  });
  listen("script:watch-jump", () => {
    eventQueue.clear();
    const gameStore = useGameStore();
    gameStore.forceChoice = null;
    gameStore.poemGame = null;
    eventQueue.resume();
  });
  listen("script:force-choice", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "force_choice", defaultDuration: 0 }));
  });
  listen("script:poem-game", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "poem_game", defaultDuration: 0 }));
  });
}

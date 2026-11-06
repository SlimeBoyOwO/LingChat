import type { GameState } from "@lingchat/host/stores/modules/game/state";
import { useUIStore } from "@lingchat/host/stores/modules/ui/ui";
import { useSettingsStore } from "@lingchat/host/stores/modules/settings";
import { invoke } from "@tauri-apps/api/core";
import { resetScriptWindowTitle } from "../utils/windowTitleCoordinator";
function clearStoryInteractionState(state: GameState) {
  state.runningScript = null;
  state.forceChoice = null;
  state.poemGame = null;
}

function restoreStoryMedia(uiStore: ReturnType<typeof useUIStore>) {
  if (uiStore.preScriptBgm !== null) {
    uiStore.currentBackgroundMusic = uiStore.preScriptBgm;
    uiStore.preScriptBgm = null;
  }
  if (uiStore.preScriptBgmMode !== null) {
    uiStore.bgMusicMode = uiStore.preScriptBgmMode;
    uiStore.preScriptBgmMode = null;
  }
  uiStore.bgmPersistBlocked = false;
  uiStore.clearAmbientTracks();
  uiStore.triggerSoundEffect("None");
  uiStore.bgMusicPlaybackRate = 1;
}

function clearStoryEffects(uiStore: ReturnType<typeof useUIStore>) {
  uiStore.resetHorrorEffects();
  resetScriptWindowTitle();
  uiStore.showPlayerHintLine = "";
}

function restoreStoryStage(state: GameState, uiStore: ReturnType<typeof useUIStore>) {
  if (uiStore.preScriptBackground !== null) {
    useSettingsStore().setCurrentBackground(uiStore.preScriptBackground);
    uiStore.preScriptBackground = null;
  }
  if (state.preScriptRoleIds !== null) {
    state.presentRoleIds = state.preScriptRoleIds;
    state.preScriptRoleIds = null;
  }
  for (const id of state.presentRoleIds) {
    const role = state.gameRoles[id];
    if (role) {
      role.show = true;
      role.emotion = "正常";
    }
  }
}

function releaseStorySystemResources(notifyBackend: boolean) {
  invoke("close_script_glitch_windows").catch((err) =>
    console.warn("[Script] 关闭故障窗口失败（非致命）:", err),
  );
  if (notifyBackend) {
    invoke("stop_script").catch((err) => console.warn("[Script] stop_script 失败（非致命）:", err));
  }
}

export function enterExtensionStory(state: GameState, uiStore: ReturnType<typeof useUIStore>) {
  state.poemGame = null;
  // 剧本 BGM 隔离：保存自由对话的 BGM 与循环模式（已在剧本中则保留最早的值），
  // 并阻止剧本期间的 BGM 变化被持久化到 session（防泄漏到下次启动）
  if (uiStore.preScriptBgm === null) {
    uiStore.preScriptBgm = uiStore.currentBackgroundMusic;
    uiStore.preScriptBgmMode = uiStore.bgMusicMode;
  }
  // 背景图同理：剧本换的崩坏背景不得残留到自由对话
  if (uiStore.preScriptBackground === null) {
    uiStore.preScriptBackground = useSettingsStore().display.currentBackground;
  }
  // 在场角色快照：剧本演出的 hide_character（结局"角色消失"）会改写
  // presentRoleIds，退出时必须恢复，否则自由对话立绘消失
  if (state.preScriptRoleIds === null) {
    state.preScriptRoleIds = [...state.presentRoleIds];
  }
  // Every story run owns a deterministic stage. Free-dialogue characters must
  // not leak into missing/empty scenes; the snapshot is restored on exit.
  state.presentRoleIds = [];
  uiStore.bgmPersistBlocked = true;
  uiStore.bgMusicMode = "loop-single";
  // 进入新剧本前清掉可能残留的恐怖特效（上次异常退出/重启等情况）
  uiStore.resetHorrorEffects();
}
export function exitExtensionStory(state: GameState, notifyBackend: boolean) {
  const uiStore = useUIStore();
  clearStoryInteractionState(state);
  restoreStoryMedia(uiStore);
  clearStoryEffects(uiStore);
  restoreStoryStage(state, uiStore);
  releaseStorySystemResources(notifyBackend);
}

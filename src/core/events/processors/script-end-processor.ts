import { invoke } from "@tauri-apps/api/core";
import type { IEventProcessor } from "../event-processor";
import type { ScriptEndEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { WebSocketMessageTypes } from "../../../types";
import { useAdventureStore } from "@/stores/modules/adventure";
import router from "@/router";
import { eventQueue } from "../event-queue";
import { resetScriptWindowTitle } from "@/utils/windowTitleCoordinator";

export default class ScriptEndProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === WebSocketMessageTypes.SCRIPT_END;
  }

  async processEvent(event: ScriptEndEvent): Promise<void> {
    // completed === false 表示剧本是因为出错被中止的。仍然要退出剧情模式把
    // UI 放出来，但不能把羁绊记为完成。旧版本没有这个字段，按完成处理。
    const completed = event.completed !== false;

    const adventureStore = useAdventureStore();
    if (completed && adventureStore.inProgressAdventures.length > 0) {
      // 按道理来讲，应该只有一个进行的剧本哈，但是为了保险起见，还是遍历一下
      for (const adventure of adventureStore.inProgressAdventures) {
        adventureStore.markAdventureCompleted(adventure.adventure_folder);
      }
    }

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

    const gameStore = useGameStore();
    const uiStore = useUIStore();
    // 只有明确标记为 horror 的 DLC 使用 DDLC 式回标题；普通/旧剧本保持
    // 既有的无缝返回自由对话语义。
    const returnToMenu = gameStore.runningScript?.contentWarning === "horror";
    gameStore.exitStoryMode();

    // 剧本声明 main_character 时，后端把进前主角随 script:end 载荷交还（不能即时
    // emit——本处理器就是队列有序性的保证）。presentRoleIds 已由 exitStoryMode 按
    // 进前快照还原，这里只交还「当前对话角色」与主界面标题。
    if (event.restoredRoleId != null) {
      const role = await gameStore.getOrCreateGameRole(event.restoredRoleId);
      gameStore.currentInteractRoleId = event.restoredRoleId;
      uiStore.showCharacterTitle = role.roleName;
      uiStore.showCharacterSubtitle = role.roleSubTitle;
    }

    uiStore.showPlayerHintLine = "";
    // 恐怖特效/突脸/显式标题不得带出剧本外。
    uiStore.resetHorrorEffects();
    resetScriptWindowTitle();

    // 恐怖 DLC 自然结束：DDLC 式 full_restart 回标题；普通剧本不改变既有结束行为。
    if (returnToMenu && router.currentRoute.value.path === "/chat") {
      await router.replace("/");
    }
  }
}

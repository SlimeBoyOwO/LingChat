import { finishExtensionStory } from "@lingchat/dlc-ui/host/finish";
import type { IEventProcessor } from "../event-processor";
import type { ScriptEndEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { WebSocketMessageTypes } from "../../../types";
import { useAdventureStore } from "@/stores/modules/adventure";

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

    const gameStore = useGameStore();
    const uiStore = useUIStore();
    const finishPresentation = await finishExtensionStory(gameStore.runningScript);
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
    await finishPresentation();
  }
}

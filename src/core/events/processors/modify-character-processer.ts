import type { IEventProcessor } from "../event-processor";
import type { ScriptModifyCharacterEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";

export default class ModifyCharacterProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "modify_character";
  }

  async processEvent(event: ScriptModifyCharacterEvent, signal?: AbortSignal): Promise<void> {
    const gameStore = useGameStore();

    console.log("执行修改角色" + event.characterId + event.emotion + event.action);
    const delay = event.duration;

    gameStore.currentStatus = "presenting";

    if (event.characterId) {
      // 闪现演出：立绘短暂换成指定情绪后自动还原，不触碰角色真实状态
      // （后续 dialogue 事件回写 emotion 也不会把闪现冲掉——闪现在组件层做覆盖层）
      if (event.flash && event.emotion) {
        // Ensure the role/folder is ready before the avatar overlay starts its
        // short resolution window; otherwise the authored beat can be missed.
        await gameStore.getOrCreateGameRole(event.characterId);
        if (signal?.aborted) return;
        useUIStore().triggerSpriteFlash(event.characterId, event.emotion, delay > 0 ? delay : 0.45);
        return;
      }

      // 噪点侵蚀演出（DDLC n_rects_ghost 式）：常驻到剧本显式 noise:none 清除；
      // 与 emotion/action 可同拍叠加，所以处理后继续走常规分支
      if (event.noise !== undefined) {
        await gameStore.getOrCreateGameRole(event.characterId);
        if (signal?.aborted) return;
        useUIStore().triggerSpriteNoise(event.characterId, event.noise, event.noiseFadeIn ?? 0);
      }

      // 确保游戏初始化包含角色
      const role = await gameStore.getOrCreateGameRole(event.characterId);
      if (signal?.aborted) return;

      if (event.action) {
        switch (event.action) {
          case "show_character":
            role.show = false; // 确保之前是不显示的 TODO 不知道这个有没有必要加
            if (!gameStore.presentRoleIds.includes(event.characterId)) {
              gameStore.presentRoleIds.push(event.characterId);
            }
            role.show = true;
            break;
          case "hide_character":
            role.show = false;
            if (delay > 0) {
              setTimeout(() => {
                gameStore.presentRoleIds = gameStore.presentRoleIds.filter(
                  (id) => id !== event.characterId
                );
              }, delay * 1000);
            } else {
              gameStore.presentRoleIds = gameStore.presentRoleIds.filter(
                (id) => id !== event.characterId
              );
            }
            break;
          default:
            break;
        }
      }

      if (event.clothes) role.clothesName = event.clothes;

      if (event.emotion) role.emotion = event.emotion;
    } else console.warn("角色修改没有角色");

    // TODO: 根据查找的角色id，修改角色状态
  }
}

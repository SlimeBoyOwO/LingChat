import type { ScriptModifyCharacterEvent } from "@lingchat/host/types";
import { useGameStore } from "@lingchat/host/stores/modules/game";
import { useUIStore } from "@lingchat/host/stores/modules/ui/ui";
/** Returns true when an extension consumes the presentation without changing the role. */
export async function applyCharacterPresentation(
  event: ScriptModifyCharacterEvent,
  signal?: AbortSignal,
): Promise<boolean> {
  const gameStore = useGameStore(),
    delay = event.duration;
  // 闪现演出：立绘短暂换成指定情绪后自动还原，不触碰角色真实状态
  // （后续 dialogue 事件回写 emotion 也不会把闪现冲掉——闪现在组件层做覆盖层）
  if (event.flash && event.emotion) {
    // Ensure the role/folder is ready before the avatar overlay starts its
    // short resolution window; otherwise the authored beat can be missed.
    await gameStore.getOrCreateGameRole(event.characterId);
    if (signal?.aborted) return true;
    useUIStore().triggerSpriteFlash(event.characterId, event.emotion, delay > 0 ? delay : 0.45);
    return true;
  }

  // 噪点侵蚀演出（DDLC n_rects_ghost 式）：常驻到剧本显式 noise:none 清除；
  // 与 emotion/action 可同拍叠加，所以处理后继续走常规分支
  if (event.noise !== undefined) {
    await gameStore.getOrCreateGameRole(event.characterId);
    if (signal?.aborted) return true;
    useUIStore().triggerSpriteNoise(event.characterId, event.noise, event.noiseFadeIn ?? 0);
  }

  return false;
}

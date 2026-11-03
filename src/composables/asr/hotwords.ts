import { asrLog } from "./log";
import { runtime, type HotwordSnapshot } from "./state";
import { getGlobalHotwords, getSceneHotwords, type RoleHotword } from "@/api/services/hotword";

/**
 * 场景热词缓存的刷新与快照（识别时注入）。
 * 刷新先作废旧缓存再 await，读回窗口内只拿到空表：宁可不加偏置，也不用错角色的词表。
 */

/** 角色名热词的权重：普通档上界 5，即「强制偏好」档。 */
const ROLE_NAME_WEIGHT = 5;

/** 主角名 + 在场角色名 → `runtime.roleNameHotwords`，主角名在前。
 *  不写库，同名去重交给后端按 text 取 max、保留首现位置。 */
function syncRoleNameHotwords(): void {
  const game = runtime.gameStore;
  if (!game) {
    runtime.roleNameHotwords = [];
    return;
  }
  const ids: number[] = [];
  if (game.mainRoleId > 0) ids.push(game.mainRoleId);
  for (const id of game.presentRoleIds) {
    if (id > 0 && !ids.includes(id)) ids.push(id);
  }
  // 用 for 循环：TS 的 filter 不做类型收窄，`role.roleName` 会报可能为 undefined
  const names: RoleHotword[] = [];
  for (const id of ids) {
    const name = game.gameRoles[id]?.roleName?.trim() ?? "";
    if (name === "") continue;
    names.push({
      text: name,
      weight: ROLE_NAME_WEIGHT,
      // -1：角色名归因，不与真实 role_id 和全局词的 0 占位冲突
      role_id: -1,
    });
  }
  runtime.roleNameHotwords = names;
}

/** 刷新场景热词缓存（主角与在场角色的热词 + 全局热词），角色或热词保存后调用。 */
export async function refreshSceneHotwords(): Promise<void> {
  const game = runtime.gameStore;
  if (!game) {
    runtime.roleNameHotwords = [];
    runtime.hotwords = [];
    return;
  }
  // 角色名同步派生，先于 await 生效，不受刷新成败影响
  syncRoleNameHotwords();
  const generation = ++runtime.hotwordsGeneration;
  runtime.hotwords = [];
  // -1（未选主角）与 0（玩家）不是热词归属角色
  const main = game.mainRoleId > 0 ? game.mainRoleId : null;
  const present = game.presentRoleIds.filter((id) => id > 0);
  try {
    const [hotwords, globalHotwords] = await Promise.all([
      getSceneHotwords(main, present),
      getGlobalHotwords(),
    ]);
    // 期间又刷新过：本次结果已过期，丢弃
    if (runtime.hotwordsGeneration !== generation) return;
    // 场景词在前、全局词在后：prompt 从尾部截断，靠前优先。同词归并取 max、保留首现位置
    runtime.hotwords = [...hotwords, ...globalHotwords];
  } catch (e) {
    // 复位 generation：否则快照校验会一直通过却是空表，整段会话都无热词
    if (runtime.hotwordsGeneration === generation) {
      runtime.hotwordsGeneration++;
    }
    // 保持空缓存，避免沿用上一角色的词表
    asrLog().warn("refresh scene hotwords failed:", e);
  }
}

/** 录音开始时取快照，角色名前置拼接（截断时最后被丢）。
 *  必须拷贝数组：下次同步会整体换新，快照需冻结当时的内容。 */
export function takeHotwordSnapshot(): HotwordSnapshot {
  return {
    generation: runtime.hotwordsGeneration,
    hotwords: [...runtime.roleNameHotwords, ...runtime.hotwords],
  };
}

/** 快照取后缓存是否又刷新过；变了说明场景已变。 */
export function isHotwordSnapshotCurrent(snapshot: HotwordSnapshot): boolean {
  return runtime.hotwordsGeneration === snapshot.generation;
}

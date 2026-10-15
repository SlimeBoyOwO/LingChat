import { asrLog } from "./log";
import { runtime, type HotwordSnapshot } from "./state";
import { getGlobalHotwords, getSceneHotwords, type RoleHotword } from "@/api/services/hotword";

/**
 * 场景热词缓存的刷新与快照（识别时注入）。
 *
 * 缓存写入是异步的（DB 往返）、读取是同步的：切角色后 `await` 还没回来时用户就按
 * PTT → 会读到**上一个角色**的词表；而流式会话中途改热词不生效、也不自动
 * 重启（会打断正在进行的识别）——整段话都会用错角色。故刷新先作废旧缓存再 await，
 * 异步读回窗口内读取方只会拿到空表：宁可不加偏置，也不用错角色的词表。
 *
 * 失败路径同样要复位 generation——见 `catch` 里的说明：不复位会让"一次失败"变成
 * "整段会话都无热词"（快照校验永远通过、内容永远为空）。
 */

/** 权重 5 = 厂商口径的普通档上界（`db/entities/hotword_weight.rs::WEIGHT_CAP`），
 *  也是角色名热词指定的「强制偏好」档。 */
const ROLE_NAME_WEIGHT = 5;

/**
 * 主角名 + 在场角色名 → `runtime.roleNameHotwords`。
 *
 * **不写库、不物化**：角色名不进 `role_hotword`，因此不走 `get_scene_hotwords`
 * 那条读库路径。改名后下次识别自动生效。
 *
 * 顺序 = 主角名在最前、在场角色名依次跟随（与「主角优先」一致）。
 * 同名去重交给 `build_options`（按 text 取 max、保留首现位置）。
 */
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
  // 用 for 循环而不是 map/filter：`gameRoles[id]` 是 `GameRole | undefined`，
  // 而 TS 的 `filter` 不做类型收窄（除非写显式谓词），`role.roleName` 会报可能为 undefined。
  const names: RoleHotword[] = [];
  for (const id of ids) {
    const name = game.gameRoles[id]?.roleName?.trim() ?? "";
    if (name === "") continue;
    names.push({
      text: name,
      weight: ROLE_NAME_WEIGHT,
      // 归因：`-1` 不与真实 role_id 冲突，也不占用全局词的 `0` 占位
      //（与《情景词集绑定》§6.1 的归因约定同一套）。
      role_id: -1,
    });
  }
  runtime.roleNameHotwords = names;
}

/** 刷新场景热词缓存（主角 ∪ 在场角色的热词 + 全局热词）。
 *  调用点：index.ts 的主角/在场角色 watch、角色设置弹窗保存成功后、全局热词弹窗保存成功后。 */
export async function refreshSceneHotwords(): Promise<void> {
  const game = runtime.gameStore;
  if (!game) {
    runtime.roleNameHotwords = [];
    runtime.hotwords = [];
    return;
  }
  // 角色名先于任何 await 同步派生：切角色后立刻生效，不经过 DB、
  // 也不受下面那次刷新成败影响。
  syncRoleNameHotwords();
  // 立即作废旧缓存（清空 + generation+1）再 await：见文件头
  const generation = ++runtime.hotwordsGeneration;
  runtime.hotwords = [];
  // -1（未选定主角的哨兵）与 0（User，玩家本体）不是热词归属角色，两个入口都滤掉
  const main = game.mainRoleId > 0 ? game.mainRoleId : null;
  const present = game.presentRoleIds.filter((id) => id > 0);
  try {
    const [hotwords, globalHotwords] = await Promise.all([
      getSceneHotwords(main, present),
      getGlobalHotwords(),
    ]);
    // 期间又刷新过（角色再变 / 连续保存）：本次结果已过期，丢弃——旧请求后到，
    // 不得覆盖新数据
    if (runtime.hotwordsGeneration !== generation) return;
    // 场景词在前、全局词追加在后：llama 的 prompt 从尾部按字符截断，靠前 = 优先。
    // 同词归并交给 build_options（按 text 取 max、保留首现位置 = 角色词的位置）
    runtime.hotwords = [...hotwords, ...globalHotwords];
  } catch (e) {
    // 复位 generation：否则这颗 +1 会一直挂到下一次成功刷新，期间**每次**录音拿到的
    // 快照都"有效"且为空表——读失败一次就静默丢掉整段会话的偏置。复位后，下一次
    // 角色变化或保存会重新刷新；在那之前读取方拿到的是空表，与失败时同样保守。
    if (runtime.hotwordsGeneration === generation) {
      runtime.hotwordsGeneration++;
    }
    // 保持空缓存：读失败宁可本次不带热词，也不能留下上一角色的词表
    asrLog().warn("refresh scene hotwords failed:", e);
  }
}

/** 录音开始时取快照（流式与非流式统一在此取）。
 *
 *  角色名**前置拼接**：它在有序列表最前 = 截断时最后被丢。
 *  ⚠️ 要拷贝数组，不能直接引用 `runtime.roleNameHotwords`——它在下一次
 *  `syncRoleNameHotwords` 时整体换新，快照必须冻结住录音开始那一刻的内容。 */
export function takeHotwordSnapshot(): HotwordSnapshot {
  return {
    generation: runtime.hotwordsGeneration,
    hotwords: [...runtime.roleNameHotwords, ...runtime.hotwords],
  };
}

/** 快照取后缓存是否又刷新过。变了 = 场景已变，快照可能不是当前角色的词表。 */
export function isHotwordSnapshotCurrent(snapshot: HotwordSnapshot): boolean {
  return runtime.hotwordsGeneration === snapshot.generation;
}

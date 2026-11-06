/** 关系就绪门的共享状态：主界面提示条/引导模态与输入框禁用共用 */
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** 缺哪一侧的文案分支 */
export type RelationMissingSide = "none" | "role" | "identity" | "both";

interface RelationReadyRole {
  role_id: number;
  role_side: boolean;
  identity_side: boolean;
}

interface RelationReadyStatus {
  ready: boolean;
  roles: RelationReadyRole[];
}

export interface BlockedRelationRole {
  roleId: number;
  missingSide: RelationMissingSide;
}

/** 当前角色与身份的关系是否已就绪。未判定过时按就绪处理，避免误拦 */
const ready = ref(true);
const missingSide = ref<RelationMissingSide>("none");
const blockedRoleId = ref<number | null>(null);
const blockedRoles = ref<BlockedRelationRole[]>([]);
const modalVisible = ref(false);

function resolveMissingSide(roleSide: boolean, identitySide: boolean): RelationMissingSide {
  if (roleSide && identitySide) return "none";
  if (!roleSide && !identitySide) return "both";
  return roleSide ? "identity" : "role";
}

function queryStatus(roleId: number): Promise<RelationReadyStatus> {
  return invoke<RelationReadyStatus>("check_relation_ready", { roleId });
}

function applyStatus(status: RelationReadyStatus) {
  const pending = (status.roles ?? [])
    .map((item) => ({
      roleId: item.role_id,
      missingSide: resolveMissingSide(item.role_side, item.identity_side),
    }))
    .filter((item) => item.missingSide !== "none");

  ready.value = status.ready;
  blockedRoles.value = status.ready ? [] : pending;
  missingSide.value = blockedRoles.value[0]?.missingSide ?? "none";
  blockedRoleId.value = status.ready ? null : (blockedRoles.value[0]?.roleId ?? null);
}

function resetReadyState() {
  ready.value = true;
  missingSide.value = "none";
  blockedRoleId.value = null;
  blockedRoles.value = [];
}

async function refresh(roleId: number | null | undefined) {
  if (!roleId || roleId <= 0) {
    resetReadyState();
    return;
  }
  try {
    applyStatus(await queryStatus(roleId));
  } catch (error) {
    console.error("[RelationGuard] 检查关系就绪失败:", error);
    resetReadyState();
  }
}

/** 后端兜底拦下时调用：标记未就绪并弹出引导模态 */
function block(roleId: number | null | undefined) {
  ready.value = false;
  missingSide.value = "both";
  blockedRoles.value = roleId && roleId > 0 ? [{ roleId, missingSide: "both" }] : [];
  if (roleId && roleId > 0) {
    blockedRoleId.value = roleId;
    void queryStatus(roleId)
      .then((status) => {
        applyStatus(status);
      })
      .catch(() => {});
  }
  modalVisible.value = true;
}

function openModal() {
  if (!ready.value) modalVisible.value = true;
}

function closeModal() {
  modalVisible.value = false;
}

export function useRelationGuard() {
  return {
    ready,
    missingSide,
    blockedRoleId,
    blockedRoles,
    modalVisible,
    refresh,
    block,
    openModal,
    closeModal,
  };
}

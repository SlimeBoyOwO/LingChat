import { invoke } from "@tauri-apps/api/core";
import type { WebInitData } from "./game-info";

export interface PlayerIdentity {
  /** 玩家角色 id；新建时为 null，由后端建角色行并回填 */
  role_id: number | null;
  name: string;
  subtitle: string;
  prompt: string;
  avatar?: string | null;
  /** 我对其他角色的认知：键为目标角色 id 的字符串形式 */
  relations: Record<string, string>;
}

export interface PlayerIdentitySummary {
  role_id: number;
  name: string;
  subtitle: string;
  prompt: string;
  has_avatar: boolean;
  relation_count: number;
  is_current: boolean;
  is_primary: boolean;
}

export const emptyPlayerIdentity = (): PlayerIdentity => ({
  role_id: null,
  name: "",
  subtitle: "",
  prompt: "",
  avatar: null,
  relations: {},
});

export const listPlayerIdentities = async (): Promise<PlayerIdentitySummary[]> => {
  return await invoke<PlayerIdentitySummary[]>("list_player_identities");
};

export const getPlayerIdentity = async (roleId: number): Promise<PlayerIdentity | null> => {
  return await invoke<PlayerIdentity | null>("get_player_identity", { roleId });
};

export const savePlayerIdentity = async (identity: PlayerIdentity): Promise<PlayerIdentity> => {
  return await invoke<PlayerIdentity>("save_player_identity", { identity });
};

export const deletePlayerIdentity = async (roleId: number): Promise<void> => {
  await invoke("delete_player_identity", { roleId });
};

export const startNewGameWithIdentity = async (roleId: number): Promise<WebInitData> => {
  return await invoke<WebInitData>("start_new_game_with_identity", { roleId });
};

/** 原地切换当前身份：历史保留、角色收到切换旁白；转场描述可留空（留空则不生成）。 */
export const setCurrentPlayerIdentity = (roleId: number, exitDesc = "", enterDesc = "") =>
  invoke<WebInitData>("set_current_player_identity", { roleId, exitDesc, enterDesc });

/** 关系与初始好感度都以目标角色 id 为键；初始好感度只对玩家角色目标有意义 */
export interface RoleRelationsPayload {
  relations: Record<string, string>;
  affections: Record<string, number | null>;
}

export const getRoleRelations = async (roleId: number): Promise<RoleRelationsPayload> => {
  return await invoke<RoleRelationsPayload>("get_role_relations", { roleId });
};

export const saveRoleRelations = async (
  roleId: number,
  relations: Record<string, string>,
  affections: Record<string, number | null>,
): Promise<void> => {
  await invoke("save_role_relations", { roleId, relations, affections });
};

/** 关系目标键：统一用角色 id 的字符串形式。 */
export const roleKey = (roleId: number | string) => String(roleId);

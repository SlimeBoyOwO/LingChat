import { invoke } from "@tauri-apps/api/core";
import type { WebInitData } from "./game-info";

export interface PlayerIdentity {
  /** 不可变 id；新建时传空串，由后端生成 */
  id: string;
  name: string;
  subtitle: string;
  prompt: string;
  avatar?: string | null;
  /** 关系表：键为 `ai:<角色文件夹>` 或 `me:<身份 id>` */
  relations: Record<string, string>;
}

export interface PlayerIdentitySummary {
  id: string;
  name: string;
  subtitle: string;
  prompt: string;
  has_avatar: boolean;
  relation_count: number;
  is_current: boolean;
}

export const emptyPlayerIdentity = (): PlayerIdentity => ({
  id: "",
  name: "",
  subtitle: "",
  prompt: "",
  avatar: null,
  relations: {},
});

export const listPlayerIdentities = async (): Promise<PlayerIdentitySummary[]> => {
  return await invoke<PlayerIdentitySummary[]>("list_player_identities");
};

export const getPlayerIdentity = async (id: string): Promise<PlayerIdentity | null> => {
  return await invoke<PlayerIdentity | null>("get_player_identity", { id });
};

export const savePlayerIdentity = async (identity: PlayerIdentity): Promise<PlayerIdentity> => {
  return await invoke<PlayerIdentity>("save_player_identity", { identity });
};

export const deletePlayerIdentity = async (id: string): Promise<void> => {
  await invoke("delete_player_identity", { id });
};

export const startNewGameWithIdentity = async (id: string): Promise<WebInitData> => {
  return await invoke<WebInitData>("start_new_game_with_identity", { id });
};

/** 原地切换当前身份：历史保留、角色收到切换旁白；转场描述可留空（留空则不生成）。 */
export const setCurrentPlayerIdentity = (id: string, exitDesc = "", enterDesc = "") =>
  invoke<WebInitData>("set_current_player_identity", { id, exitDesc, enterDesc });

/** 关系文本按 `ai:` / `me:` 键；初始好感度按 `me:<身份 id>` 键，null = 跟随角色卡默认 */
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

export const aiKey = (characterFolder: string) => `ai:${characterFolder}`;
export const meKey = (identityId: string) => `me:${identityId}`;

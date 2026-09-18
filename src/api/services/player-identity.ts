import { invoke } from "@tauri-apps/api/core";
import type { WebInitData } from "./game-info";

/** 身份卡（对应后端 `PlayerIdentity`） */
export interface PlayerIdentity {
  /** 不可变 id；新建时传空串，由后端生成 */
  id: string;
  name: string;
  subtitle: string;
  /** 注入聊天上下文的身份提示词 */
  prompt: string;
  avatar?: string | null;
  /** 关系表：键为 `ai:<角色文件夹>` 或 `me:<身份 id>` */
  relations: Record<string, string>;
}

/** 身份列表项（对应后端 `PlayerIdentitySummary`） */
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

export const getCurrentPlayerIdentity = async (): Promise<PlayerIdentity> => {
  return await invoke<PlayerIdentity>("get_current_player_identity");
};

/** 新建或更新身份卡。`identity.id` 为空视为新建。 */
export const savePlayerIdentity = async (identity: PlayerIdentity): Promise<PlayerIdentity> => {
  return await invoke<PlayerIdentity>("save_player_identity", { identity });
};

/** 删除身份卡。被存档引用时后端会拒绝。 */
export const deletePlayerIdentity = async (id: string): Promise<void> => {
  await invoke("delete_player_identity", { id });
};

/**
 * 用指定身份**开一段新对话** —— 换身份的正式路径。
 *
 * 会清空当前对话的记忆、并按新身份重建人设行（与切换 AI 角色同一条路径），
 * 同时解除本局与任何存档的绑定。旧对话想留着请先到存档页建档：
 * 存档记录的是当时的身份，读它会连身份一起恢复。
 */
export const startNewGameWithIdentity = async (id: string): Promise<WebInitData> => {
  return await invoke<WebInitData>("start_new_game_with_identity", { id });
};

export const getRoleRelations = async (roleId: number): Promise<Record<string, string>> => {
  return await invoke<Record<string, string>>("get_role_relations", { roleId });
};

export const saveRoleRelations = async (
  roleId: number,
  relations: Record<string, string>
): Promise<void> => {
  await invoke("save_role_relations", { roleId, relations });
};

/** 关系键工具：与后端 `RelationEndpoint` 的命名空间保持一致 */
export const aiKey = (characterFolder: string) => `ai:${characterFolder}`;
export const meKey = (identityId: string) => `me:${identityId}`;

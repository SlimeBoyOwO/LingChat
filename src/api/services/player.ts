import { invoke } from "@tauri-apps/api/core";
import type { PlayerProfile } from "./game-info";

/**
 * 读取全局玩家档案（当前激活人设，纯 DB 存储）。
 */
export const getPlayerProfile = async (): Promise<PlayerProfile> => {
  return await invoke<PlayerProfile>("get_player_profile");
};

/**
 * 读取指定人设卡的内容（编辑非激活人设卡时使用）。
 */
export const getPlayerPersona = async (card_id: string): Promise<PlayerProfile> => {
  return await invoke<PlayerProfile>("get_player_persona", { cardId: card_id });
};

/**
 * 保存到指定人设卡（不触碰激活位与运行时；仅当目标是当前激活人设时，
 * 应改用 setPlayerProfile 以获得运行时热更新）。
 */
export const setPlayerPersona = async (
  card_id: string,
  user_name: string,
  user_subtitle?: string,
  user_prompt?: string,
  info?: string,
  system_prompt_example?: string
): Promise<{ success: boolean }> => {
  return await invoke<{ success: boolean }>("set_player_persona", {
    cardId: card_id,
    userName: user_name,
    userSubtitle: user_subtitle,
    userPrompt: user_prompt,
    info,
    systemPromptExample: system_prompt_example,
  });
};

/**
 * 保存全局玩家档案（写入当前激活人设）。
 */
export const setPlayerProfile = async (
  user_name: string,
  user_subtitle?: string,
  user_prompt?: string,
  info?: string,
  system_prompt_example?: string
): Promise<{ success: boolean }> => {
  return await invoke<{ success: boolean }>("set_player_profile", {
    userName: user_name,
    userSubtitle: user_subtitle,
    userPrompt: user_prompt,
    info,
    systemPromptExample: system_prompt_example,
  });
};

/** 玩家人设卡摘要 */
export interface PlayerPersonaSummary {
  card_id: string;
  user_name: string;
  active: boolean;
}

/** 列出所有玩家人设卡（含当前激活人设） */
export const getPlayerProfiles = async (): Promise<{
  profiles: PlayerPersonaSummary[];
  active_profile_id: string;
}> => {
  return await invoke<{ profiles: PlayerPersonaSummary[]; active_profile_id: string }>(
    "get_player_profiles"
  );
};

/** 切换当前激活人设 */
export const setActivePlayerCard = async (card_id: string): Promise<{ success: boolean }> => {
  return await invoke<{ success: boolean }>("set_active_player_card", { cardId: card_id });
};

/** 新建一张玩家人设卡 */
export const createPlayerCard = async (
  card_id: string,
  user_name: string,
  user_subtitle?: string,
  user_prompt?: string,
  info?: string,
  system_prompt_example?: string
): Promise<{ success: boolean }> => {
  return await invoke<{ success: boolean }>("create_player_card", {
    cardId: card_id,
    userName: user_name,
    userSubtitle: user_subtitle,
    userPrompt: user_prompt,
    info,
    systemPromptExample: system_prompt_example,
  });
};

/** 删除一张玩家人设卡（禁止删除当前激活人设） */
export const deletePlayerCard = async (card_id: string): Promise<{ success: boolean }> => {
  return await invoke<{ success: boolean }>("delete_player_card", { cardId: card_id });
};

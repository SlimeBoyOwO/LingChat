import http from "../http";

// 1. 定义角色配置接口 (原先摊平的字段现在归属到这里)
export interface CharacterSettings {
  ai_name: string;
  ai_subtitle: string;
  user_name: string;
  user_subtitle: string;
  character_id: number;
  thinking_message: string;
  scale: number;
  offset_x: number;
  offset_y: number;
  bubble_top: number;
  bubble_left: number;
  clothes: Record<string, any>;
  clothes_name: string;
  body_part: Record<string, any>;
}

// 2. 定义完整的初始化数据接口 (对应后端的 WebInitData)
export interface WebInitData {
  character_settings: CharacterSettings;
  current_interact_role_id: number | null;
  onstage_roles_ids: number[];
  background: string;
  background_effect: string;
  background_music: string;
}

/**
 * 获取游戏初始化信息
 * @param client_id 客户端唯一标识
 * @param userId 用户ID
 */
export const getGameInfo = async (
  client_id: string,
  userId: string,
): Promise<WebInitData> => {
  try {
    const data = await http.get("/v1/chat/info/init", {
      params: { client_id: client_id, user_id: userId },
    });

    console.log("成功获取初始化数据:", data);
    return data;
  } catch (error: any) {
    console.error("获取初始化信息错误:", error.message);
    throw error;
  }
};

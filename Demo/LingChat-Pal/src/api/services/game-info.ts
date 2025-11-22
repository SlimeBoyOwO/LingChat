import http from "../http";

export interface GameInfo {
  ai_name: string;
  ai_subtitle: string;
  user_name: string;
  user_subtitle: string;
  character_id: number;
  thinking_message: string;
  scale: number;
  offset: number;
  bubble_top: number;
  bubble_left: number;
}

export const getGameInfo = async (userId: string): Promise<GameInfo> => {
  try {
    // 拦截器已解构数据，response.data 直接就是 GameInfo
    const data = await http.get("/v1/chat/info/init", {
      params: { user_id: userId },
    });
    console.log(data); // 直接输出 GameInfo 数据
    return data;
  } catch (error: any) {
    console.error("获取游戏信息错误:", error.message);
    throw error; // 直接抛出拦截器处理过的错误
  }
};

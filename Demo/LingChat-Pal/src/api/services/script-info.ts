import http from "../http";

export interface CharacterSettings {
  ai_name: string;
  ai_subtitle: string;
  thinking_message: string;
  scale: number;
  offset_x: number;
  offset_y: number;
  bubble_top: number;
  bubble_left: number;
}

export interface ScriptInfo {
  script_name: string;
  characters: {
    [character_id: string]: CharacterSettings;
  };
}

export const getScriptInfo = async (
  scriptName: string
): Promise<ScriptInfo> => {
  try {
    // 拦截器已解构数据，response.data 直接就是 ScriptInfo
    const data = await http.get(`/v1/chat/script/init_script/${scriptName}`);
    console.log("Script信息:", data); // 直接输出 ScriptInfo 数据
    return data;
  } catch (error: any) {
    console.error("获取脚本信息错误:", error.message);
    throw error; // 直接抛出拦截器处理过的错误
  }
};

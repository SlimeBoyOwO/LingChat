import http from "../http";
import type { Character, CharacterSelectParams } from "../../types";

export const characterGetAll = async (): Promise<Character[]> => {
  try {
    const data = await http.get("/v1/chat/character/get_all_characters");
    return data;
  } catch (error: any) {
    throw new Error(error.response?.data?.message || "获取角色列表失败");
  }
};

export const characterSelect = async (
  params: CharacterSelectParams
): Promise<Character> => {
  try {
    const response = await http.post(
      "/v1/chat/character/select_character",
      params
    );
    return response.data;
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || "角色选择失败");
  }
};

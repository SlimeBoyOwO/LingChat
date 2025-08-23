import http from "../http";
import type {
  SaveInfo,
  SaveListParams,
  SaveModifyParams,
  SaveCreateParams,
} from "../../types";

export interface SaveListData {
  conversations: SaveInfo[];
  total: number;
}

export const saveGetAll = async (
  params: SaveListParams
): Promise<SaveListData> => {
  try {
    const data = await http.get("/v1/chat/history/list", {
      params: params,
    });
    return data;
  } catch (error: any) {
    throw new Error(error.response?.data?.message || "获取存档列表失败");
  }
};

export const saveCreate = async (params: SaveCreateParams): Promise<void> => {
  try {
    await http.post("/v1/chat/history/create", params);
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || "存档创建失败");
  }
};

export const saveLoad = async (params: SaveModifyParams): Promise<void> => {
  try {
    const data = await http.get("/v1/chat/history/load", {
      params: params,
    });
    return data;
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || "存档加载失败");
  }
};

export const saveGameSave = async (params: SaveModifyParams): Promise<void> => {
  try {
    await http.post("/v1/chat/history/save", params);
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || "存档保存失败");
  }
};

export const saveDelete = async (params: SaveModifyParams): Promise<void> => {
  try {
    await http.post("/v1/chat/history/delete", params);
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || "存档删除失败");
  }
};

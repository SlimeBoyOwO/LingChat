import { invoke } from "@tauri-apps/api/core";
import http from "../http";
import type { BackgroundImageInfo } from "../../types";

export const getBackgroundImages = async (): Promise<BackgroundImageInfo[]> => {
  try {
    const data = await invoke("get_background_list");
    return data as BackgroundImageInfo[];
  } catch (error: any) {
    console.error(
      "Failed to get background list:",
      typeof error === "string" ? error : error.message
    );
    throw error;
  }
};

export const getBackgroundImageById = async (id: string): Promise<BackgroundImageInfo> => {
  return http.get(`/backgrounds/${id}`);
};

export const uploadBackgroundImage = async (
  fileName: string,
  fileData: Uint8Array,
  category?: string
): Promise<BackgroundImageInfo[]> => {
  return invoke("upload_background_image", { fileName, fileData, category });
};

export const setCurrentBackground = async (background: string): Promise<void> => {
  await http.post("/v1/chat/background/select", { background });
};

export const setCurrentBackgroundEffect = async (effect: string): Promise<void> => {
  await http.post("/v1/chat/background/effect", { effect });
};

export const generateBackgroundImage = async (prompt: string, clientId: string): Promise<void> => {
  await http.post("/v1/chat/background/generate", {
    prompt,
    client_id: clientId,
  });
};

export const openBackgroundsFolder = async (): Promise<void> => {
  await invoke("open_backgrounds_folder");
};

/** 列出所有背景子分类（子文件夹名），供分类选项卡使用 */
export const listBackgroundCategories = async (): Promise<string[]> => {
  try {
    const data = await invoke<string[]>("list_background_categories");
    return data || [];
  } catch (error: any) {
    console.error(
      "Failed to list background categories:",
      typeof error === "string" ? error : error.message
    );
    return [];
  }
};

/** 新建一个背景子分类（子文件夹） */
export const createBackgroundCategory = async (name: string): Promise<void> => {
  await invoke("create_background_category", { name });
};

/**
 * 删除一个背景子分类。
 * @param name 子分类（子文件夹）名
 * @param mode 'move_to_root' 把该分类下背景移到根目录；'delete_all' 全部删除
 * @returns 受影响的背景数量
 */
export const deleteBackgroundCategory = async (
  name: string,
  mode: "move_to_root" | "delete_all"
): Promise<number> => {
  const data = await invoke<number>("delete_background_category", { name, mode });
  return data ?? 0;
};

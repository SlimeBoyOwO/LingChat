import { invoke } from "@tauri-apps/api/core";
import http from "../http";
import type { MusicTrack } from "../../types";

export const musicGetAll = async (): Promise<MusicTrack[]> => {
  try {
    const data = await invoke("get_music_list");
    return data as MusicTrack[];
  } catch (error: any) {
    console.error("Failed to get music list:", typeof error === "string" ? error : error.message);
    throw error;
  }
};

export interface UploadMusicResult {
  actual_name: string;
  original_name: string;
  detected_kind: string;
  was_corrected: boolean;
}

export const musicUpload = async (
  path: string,
  fileName: string,
  category?: string
): Promise<UploadMusicResult> => {
  try {
    return await invoke<UploadMusicResult>("upload_music", { path, fileName, category });
  } catch (error: any) {
    throw new Error(typeof error === "string" ? error : error.message || "Music upload failed");
  }
};

/** 列出所有音乐子分类（子文件夹名），供分类选项卡使用 */
export const musicListCategories = async (): Promise<string[]> => {
  try {
    const data = await invoke<string[]>("list_music_categories");
    return data || [];
  } catch (error: any) {
    console.error(
      "Failed to list music categories:",
      typeof error === "string" ? error : error.message
    );
    return [];
  }
};

/** 新建一个音乐子分类（子文件夹） */
export const musicCreateCategory = async (name: string): Promise<void> => {
  await invoke("create_music_category", { name });
};

/** 删除一个音乐子分类，返回受影响数量 */
export const musicDeleteCategory = async (name: string, mode = "move"): Promise<number> => {
  const data = await invoke<number>("delete_music_category", { name, mode });
  return data ?? 0;
};

/** 打开音乐所在文件夹 */
export const openMusicFolder = async (): Promise<void> => {
  await invoke("open_music_folder");
};

export const musicDelete = async (url: string): Promise<void> => {
  try {
    await invoke("delete_music", { url });
  } catch (error: any) {
    throw new Error(typeof error === "string" ? error : error.message || "Music delete failed");
  }
};

export const setCurrentBackgroundMusic = async (music: string): Promise<void> => {
  await http.post("/v1/chat/back-music/select", { music });
};

/** 持久化背景音乐状态到 settings.json，下次启动时自动恢复 */
export const saveBgmState = async (track: string, paused: boolean, mode: string): Promise<void> => {
  try {
    await invoke("save_bgm_state", { track, paused, mode });
  } catch (error: any) {
    console.warn("持久化BGM状态失败（非致命）:", typeof error === "string" ? error : error.message);
  }
};

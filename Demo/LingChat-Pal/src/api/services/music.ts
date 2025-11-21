import http from "../http";
import { MusicTrack } from "../../types";

export const musicGetAll = async (): Promise<MusicTrack[]> => {
  try {
    const data = await http.get("/v1/chat/back-music/list");
    return data;
  } catch (error: any) {
    throw new Error(error.response?.data?.message || "获取音乐列表失败");
  }
};

export const musicUpload = async (formData: FormData): Promise<void> => {
  try {
    await http.post("/v1/chat/back-music/upload", formData);
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || "存档上传失败");
  }
};

export const musicDelete = async (url: string): Promise<void> => {
  try {
    await http.delete("/v1/chat/back-music/delete", {
      params: { url: url },
    });
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || "存档上传失败");
  }
};

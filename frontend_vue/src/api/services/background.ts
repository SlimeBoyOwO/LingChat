import http from "../http";
import { BackgroundImageInfo } from "../../types";

export const getBackgroundImages = async (): Promise<BackgroundImageInfo[]> => {
  try {
    const data = await http.get("/v1/chat/background/list", {});
    return data;
  } catch (error: any) {
    console.error("获取游戏信息错误:", error.message);
    throw error; // 直接抛出拦截器处理过的错误
  }
};

export const getBackgroundImageById = async (
  id: string
): Promise<BackgroundImageInfo> => {
  return http.get(`/backgrounds/${id}`);
};

export const uploadBackgroundImage = async (
  file: File
): Promise<BackgroundImageInfo> => {
  const formData = new FormData();
  formData.append("file", file);
  return http.post("/backgrounds/upload", formData, {
    headers: {
      "Content-Type": "multipart/form-data",
    },
  });
};

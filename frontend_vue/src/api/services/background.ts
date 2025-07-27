import http from "../http";
import { BackgroundImage } from "../types/background";

export const getBackgroundImages = async (): Promise<BackgroundImage[]> => {
  return http.get("/backgrounds");
};

export const getBackgroundImageById = async (
  id: string
): Promise<BackgroundImage> => {
  return http.get(`/backgrounds/${id}`);
};

export const uploadBackgroundImage = async (
  file: File
): Promise<BackgroundImage> => {
  const formData = new FormData();
  formData.append("file", file);
  return http.post("/backgrounds/upload", formData, {
    headers: {
      "Content-Type": "multipart/form-data",
    },
  });
};

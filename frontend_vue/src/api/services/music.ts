import http from "../http";
import { MusicTrack } from "../types/music";

export const getMusicList = async (): Promise<MusicTrack[]> => {
  return http.get("/music");
};

export const searchMusic = async (query: string): Promise<MusicTrack[]> => {
  return http.get("/music/search", { params: { q: query } });
};

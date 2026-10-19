import { invoke } from "@tauri-apps/api/core";

interface SaveListLite {
  saves: { id: number; title: string }[];
}

/** 取某个存档的标题（保存前提示用）；没有进行中的存档或查询失败时返回 null。 */
export const fetchSaveTitle = async (saveId: number | null): Promise<string | null> => {
  if (!saveId) return null;
  try {
    const res = await invoke<SaveListLite>("list_saves", { page: 1, pageSize: 50 });
    return res.saves.find((s) => s.id === saveId)?.title ?? null;
  } catch {
    return null;
  }
};

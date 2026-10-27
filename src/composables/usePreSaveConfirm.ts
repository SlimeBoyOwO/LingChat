/** 保存前确认：本次保存会改写的当前存档内容，先让用户确认。 */
import { invoke } from "@tauri-apps/api/core";
import { useGameStore } from "../stores/modules/game";
import { useDialogStore } from "../stores/modules/ui/dialog";

interface SaveListLite {
  saves: { id: number; title: string }[];
}

/** 取某个存档的标题（保存前提示用）；没有进行中的存档或查询失败时返回 null。 */
async function fetchSaveTitle(saveId: number | null): Promise<string | null> {
  if (!saveId) return null;
  try {
    const res = await invoke<SaveListLite>("list_saves", { page: 1, pageSize: 50 });
    return res.saves.find((s) => s.id === saveId)?.title ?? null;
  } catch {
    return null;
  }
}

/**
 * 弹「保存会改写当前存档」确认框。
 * 两张文案由调用方给出（带 {save} 与不带存档名两版），返回用户是否确认。
 */
export async function confirmSaveEffect(
  withSave: (save: string) => string,
  noSave: string,
): Promise<boolean> {
  const gameStore = useGameStore();
  const dialogStore = useDialogStore();
  const saveTitle = await fetchSaveTitle(gameStore.activeSaveId);
  return dialogStore.confirm(saveTitle ? withSave(saveTitle) : noSave);
}

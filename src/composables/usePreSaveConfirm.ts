/** 保存前确认：本次保存会改写的当前存档内容，先让用户确认。 */
import { useGameStore } from "../stores/modules/game";
import { useDialogStore } from "../stores/modules/ui/dialog";
import { fetchSaveTitle } from "../api/services/save";

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

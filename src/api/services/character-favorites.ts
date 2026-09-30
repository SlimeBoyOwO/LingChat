import { invoke } from "@tauri-apps/api/core";

/** 读取保存在 data/game_data/characters/favorites.json 中的角色收藏。 */
export const getCharacterFavorites = async (): Promise<number[]> => {
  return invoke<number[]>("get_character_favorites");
};

/** 保存角色收藏；文件属于 data/，会随 LAN 同步体系纳入同步范围。 */
export const saveCharacterFavorites = async (characterIds: number[]): Promise<void> => {
  await invoke("save_character_favorites", { characterIds });
};

import type { SceneInfo } from "@/api/services/scene";

/** 收藏的场景 id（有序）/ 未收藏场景的手动排序，均持久化在 localStorage */
const SCENE_FAVORED_KEY = "lingchat.scene.favored.v1";
const SCENE_ORDER_KEY = "lingchat.scene.order.v1";

function readIds(key: string): string[] {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as string[]) : [];
  } catch {
    return [];
  }
}

function writeIds(key: string, ids: string[]): void {
  try {
    localStorage.setItem(key, JSON.stringify(ids));
  } catch {
    // localStorage 不可用时静默降级：功能可用，仅失去持久化
  }
}

/**
 * 场景收藏与手动排序。
 *
 * 数据保存在 localStorage 且跨分类全局共享，因此排序面板只允许在「全部」分类下打开。
 * 显示顺序 = [收藏场景（按收藏顺序）] + [未收藏场景（按手动排序）]。
 */
export function useSceneFavorites() {
  function loadFavored(): string[] {
    return readIds(SCENE_FAVORED_KEY);
  }

  function loadOrder(): string[] {
    return readIds(SCENE_ORDER_KEY);
  }

  /** 按「收藏区 + 未收藏区」重排场景列表 */
  function applySceneOrder(list: SceneInfo[]): SceneInfo[] {
    const favored = loadFavored();
    const order = loadOrder();
    const favoredSet = new Set(favored);
    const favoredScenes = favored
      .map((id) => list.find((s) => s.id === id))
      .filter((s): s is SceneInfo => !!s);
    const unfavoredScenes = list.filter((s) => !favoredSet.has(s.id));
    const orderIndex = (id: string) => {
      const i = order.indexOf(id);
      return i === -1 ? Number.MAX_SAFE_INTEGER : i;
    };
    unfavoredScenes.sort((a, b) => orderIndex(a.id) - orderIndex(b.id));
    return [...favoredScenes, ...unfavoredScenes];
  }

  function isFavored(sceneId: string): boolean {
    return loadFavored().includes(sceneId);
  }

  /** 切换收藏，并同步维护两侧位置：取消收藏的场景回到未收藏区顶部 */
  function toggleFavorite(sceneId: string): void {
    const favored = loadFavored();
    if (favored.includes(sceneId)) {
      writeIds(
        SCENE_FAVORED_KEY,
        favored.filter((id) => id !== sceneId),
      );
      writeIds(SCENE_ORDER_KEY, [sceneId, ...loadOrder().filter((id) => id !== sceneId)]);
    } else {
      writeIds(SCENE_FAVORED_KEY, [...favored, sceneId]);
      writeIds(
        SCENE_ORDER_KEY,
        loadOrder().filter((id) => id !== sceneId),
      );
    }
  }

  /** 保存排序面板的结果：收藏区与未收藏区分别落库 */
  function saveOrder(favoredIds: string[], orderedIds: string[]): void {
    if (favoredIds.length > 0) writeIds(SCENE_FAVORED_KEY, favoredIds);
    if (orderedIds.length > 0) writeIds(SCENE_ORDER_KEY, orderedIds);
  }

  return { loadFavored, applySceneOrder, isFavored, toggleFavorite, saveOrder };
}

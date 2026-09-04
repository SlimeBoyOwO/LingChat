import { invoke } from "@tauri-apps/api/core";

export interface FilterParams {
  brightness: number;
  contrast: number;
  saturation: number;
  sepia: number;
  glow_radius: number;
  glow_color: string;
}

export interface LightingParams {
  character: FilterParams;
  background: FilterParams;
  overlay_enabled: boolean;
  blend_mode: string;
  light_x: number;
  light_y: number;
  overlay_color1: string;
  overlay_color2: string;
  overlay_radius: number;
  overlay_opacity: number;
  overlay_target: string;
}

export interface SceneInfo {
  id: string;
  scene_name: string;
  scene_description: string;
  background: string | null;
  /** 场景所属子分类（背景子文件夹名；根目录为「根目录」） */
  category: string;
  lighting: LightingParams | null;
  created_at: string;
  updated_at: string;
  /** 来源："game" 或提供该场景背景图的插件 id。 */
  source?: string;
  plugin_id?: string | null;
}

export interface CreateSceneRequest {
  scene_name: string;
  scene_description: string;
  background: string;
  lighting?: LightingParams | null;
}

export interface UpdateSceneRequest {
  id: string;
  scene_name: string;
  scene_description: string;
  background: string;
  lighting?: LightingParams | null;
}

export async function listScenes(): Promise<SceneInfo[]> {
  return invoke<SceneInfo[]>("list_scenes");
}

export async function createScene(req: CreateSceneRequest): Promise<SceneInfo> {
  return invoke<SceneInfo>("create_scene", { req });
}

export async function updateScene(req: UpdateSceneRequest): Promise<SceneInfo> {
  return invoke<SceneInfo>("update_scene", { req });
}

export async function deleteScene(id: string): Promise<void> {
  return invoke("delete_scene", { id });
}

/** 一键清除空白场景：删除背景已不存在的场景，返回删除数量 */
export async function clearEmptyScenes(): Promise<number> {
  const data = await invoke<number>("clear_empty_scenes");
  return data ?? 0;
}

export async function selectScene(sceneId: string | null): Promise<void> {
  return invoke("select_scene", { sceneId });
}

export async function setSceneAwareness(enabled: boolean): Promise<void> {
  return invoke("set_scene_awareness", { enabled });
}

/**
 * 把场景的背景图片移动到指定子分类（子文件夹）下，并更新场景的分类。
 * `category` 传「根目录」表示移回背景根目录。
 */
export async function moveSceneToCategory(id: string, category: string): Promise<SceneInfo> {
  return invoke<SceneInfo>("move_scene_to_category", { id, category });
}

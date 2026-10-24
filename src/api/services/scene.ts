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

/** 场景室内外标注（LLM 建场景时自动标注一次，场景卡上可点击修改）。 */
export type SceneLocationType = "indoor" | "outdoor" | "unknown";

export interface SceneInfo {
  id: string;
  scene_name: string;
  scene_description: string;
  background: string | null;
  /** 场景所属子分类；null 表示背景位于根目录或未设置分类。 */
  category: string | null;
  lighting: LightingParams | null;
  created_at: string;
  updated_at: string;
  /** 来源："game" 或提供该场景背景图的插件 id。 */
  source?: string;
  plugin_id?: string | null;
  /** 室内外标注；缺省（旧场景）按 unknown 处理。 */
  location_type?: SceneLocationType | null;
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

/** 用户手动选择粒子特效：同步后端状态并落下"手动优先"锁（AI 不再自动切特效）。 */
export async function selectBackgroundEffect(effect: string): Promise<void> {
  return invoke("select_background_effect", { effect });
}

export async function setSceneAwareness(enabled: boolean): Promise<void> {
  return invoke("set_scene_awareness", { enabled });
}

/** 手动设置场景室内外标注（场景卡点击标注时调用）。 */
export async function setSceneLocationType(
  id: string,
  locationType: SceneLocationType,
): Promise<void> {
  return invoke("set_scene_location_type", { id, locationType });
}

/** 天气同步配置（城市 + 自动同步开关）。 */
export interface WeatherSyncConfig {
  city: string;
  auto_sync: boolean;
}

export async function getWeatherSyncConfig(): Promise<WeatherSyncConfig> {
  return invoke<WeatherSyncConfig>("get_weather_sync_config");
}

/** 保存天气城市；清空城市时后端会一并关闭自动同步。 */
export async function setWeatherCity(city: string): Promise<void> {
  return invoke("set_weather_city", { city });
}

/** 切换「自动同步现实天气」；城市为空时后端拒绝打开。 */
export async function setWeatherAutoSync(enabled: boolean): Promise<void> {
  return invoke("set_weather_auto_sync", { enabled });
}

/**
 * 把场景的背景图片移动到指定子分类（子文件夹）下，并更新场景的分类。
 * `category` 传 null 表示移回背景根目录。
 */
export async function moveSceneToCategory(id: string, category: string | null): Promise<SceneInfo> {
  return invoke<SceneInfo>("move_scene_to_category", { id, category });
}

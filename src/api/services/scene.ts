import { invoke } from "@tauri-apps/api/core";

export interface FilterParams {
  brightness: number;
  contrast: number;
  saturation: number;
  sepia: number;
  glow_radius: number;
  glow_color: string;
}

export type LightType = "point" | "directional" | "spot" | "ambient";

/**
 * 单盏光源（2.5D 多光源扩展）。
 *
 * 统一扁平结构：所有几何字段都用 number 存放，不适用的字段由前端填默认值，
 * 与 Rust 侧 `LightSource` 保持一致。
 */
export interface LightSource {
  id: string;
  /** 显示名称（编辑器里给灯起的名字，缺省由前端按类型生成） */
  name?: string;
  type: LightType;
  enabled: boolean;
  /** 主光标记：纯 UI 语义，不参与光照计算，但影响角色侧的主方向权重。 */
  is_key?: boolean;
  color: string;
  intensity: number;
  x: number;
  y: number;
  z: number;
  radius: number;
  softness: number;
  azimuth: number;
  elevation: number;
  cone: number;
  penumbra: number;
}

/** 全局光影强度参数，对应 Rust `GlobalParams`。 */
export interface LightingGlobalParams {
  master: number;
  ambient_blend: number;
  albedo_protect: number;
  soft_knee: number;
  lift: number;
  mono: number;
  /** 饱和度（0–2，1 = 原色，0 = 灰度）。旧存档缺失时按 1 处理。 */
  saturation?: number;
  /** 对比度（0–2，1 = 原样）。 */
  contrast?: number;
  /** 整体色调偏移角度（度，−180–180），配合 tint_amount 使用。 */
  tint_hue?: number;
  /** 色调偏移强度（0–1）。 */
  tint_amount?: number;
  /** 立绘单独曝光增益（0–2），与 master 相乘。 */
  char_exposure?: number;
  /** 背景单独曝光增益（0–2），与 master 相乘。 */
  bg_exposure?: number;
}

/** 合成层参数（光包裹 / 边缘光 / 泛光），对应 Rust `CompositingParams`。 */
export interface LightingCompositingParams {
  light_wrap: number;
  rim_width: number;
  bloom_int: number;
  bloom_thresh: number;
  /** 泛光染色（hex），白色等于不染色。 */
  glow_color?: string;
}

/** 材质参数（伪法线 / 卡通着色 / 高光 / 自阴影），对应 Rust `MaterialParams`。 */
export interface LightingMaterialParams {
  use_normal: boolean;
  normal_strength: number;
  cel_thresh: number;
  cel_softness: number;
  spec_int: number;
  spec_gloss: number;
  self_shadow_int: number;
  use_depth: boolean;
}

export type LightingShot = "fs" | "ws" | "ms" | "cu" | "ecu";

/** 相机 / 景别参数，对应 Rust `CameraParams`。 */
export interface LightingCameraParams {
  shot: LightingShot;
  zoom: number;
  focus_y: number;
  dof: number;
}

/**
 * 一整套 2.5D 光影参数。
 *
 * 场景默认那套直接摊在 LightingParams 上，角色专属那套按 roleId 存在 role_sets 里，
 * 两边共用这个形状，工作台切换编辑目标时整存整取。
 */
export interface LightingSet {
  lights: LightSource[];
  global: LightingGlobalParams;
  compositing: LightingCompositingParams;
  material: LightingMaterialParams;
  camera: LightingCameraParams;
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
  /** 多光源数组；缺省或 null 表示沿用 CSS 滤镜 + 叠加层老逻辑。 */
  lights?: LightSource[] | null;
  /** 全局强度；缺省或 null 表示未启用 2.5D 管线。 */
  global?: LightingGlobalParams | null;
  /** 合成层参数。 */
  compositing?: LightingCompositingParams | null;
  /** 材质参数（伪法线 / 卡通着色）。 */
  material?: LightingMaterialParams | null;
  /** 相机 / 景别参数。 */
  camera?: LightingCameraParams | null;
  /**
   * 角色专属光影，键是 roleId 的字符串形式。
   *
   * 单角色对话时连背景一起用这套；多角色同台时背景恒用场景默认，这套只作用于
   * 该角色自己的立绘。没配的角色回落场景默认那套。
   */
  role_sets?: Record<string, LightingSet> | null;
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

/** 用户手动选择天气层特效：只同步后端状态，供下一轮对话检测天气变化。 */
export async function selectWeatherEffect(effect: string): Promise<void> {
  return invoke("select_weather_effect", { effect });
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

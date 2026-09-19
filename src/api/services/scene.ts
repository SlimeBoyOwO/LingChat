import { invoke } from "@tauri-apps/api/core";

export interface FilterParams {
  brightness: number;
  contrast: number;
  saturation: number;
  sepia: number;
  glow_radius: number;
  glow_color: string;
  /** 轮廓光（rim light）：沿立绘 alpha 剪影描一圈受光边 */
  rim_enabled?: boolean;
  rim_color?: string;
  /** 水平偏移 px，正值向右——决定受光侧 */
  rim_dx?: number;
  /** 垂直偏移 px，负值向上 */
  rim_dy?: number;
  rim_blur?: number;
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

  // ---------- 进阶光影层（缺省字段由后端 serde default 补齐） ----------
  /** 方向双层光：受光侧暖、背光侧冷 */
  directional_enabled?: boolean;
  /** 光的入射角 (0–360)，0 = 正上方来光，顺时针 */
  light_angle?: number;
  light_warm_color?: string;
  shadow_cool_color?: string;
  /** 明暗过渡柔和度 0–1 */
  light_softness?: number;
  /** 方向光强度 0–1 */
  light_strength?: number;
  /** bloom 泛光：背景副本模糊 + screen 混合 */
  bloom_enabled?: boolean;
  bloom_radius?: number;
  bloom_intensity?: number;
  vignette_enabled?: boolean;
  vignette_strength?: number;
  /** 暗角中心清晰区半径 % */
  vignette_size?: number;
  /** 冷暖分离染色（mix-blend-mode: color，不改变亮度） */
  grade_enabled?: boolean;
  grade_warm_color?: string;
  grade_cool_color?: string;
  grade_strength?: number;
  /** 呼吸动画：光强随时间缓慢起伏 */
  breathing_enabled?: boolean;
  /** 呼吸周期（秒） */
  breathing_period?: number;
  /** 呼吸幅度 0–1 */
  breathing_amount?: number;
}

export interface SceneInfo {
  id: string;
  scene_name: string;
  scene_description: string;
  background: string | null;
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

export async function selectScene(sceneId: string | null): Promise<void> {
  return invoke("select_scene", { sceneId });
}

export async function setSceneAwareness(enabled: boolean): Promise<void> {
  return invoke("set_scene_awareness", { enabled });
}

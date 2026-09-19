import { invoke } from "@tauri-apps/api/core";
import type { LightingParams } from "./scene";

/** 预设元信息 + 完整参数（后端 lighting_store 是唯一真源）。 */
export interface LightingPreset {
  id: string;
  name: string;
  description: string;
  /** 适用心情关键词，供搜索 / LLM 匹配 */
  mood: string[];
  params: LightingParams;
}

/** `lighting:change` / `script:lighting` 的负载，字段名与 Rust 的 camelCase 一致。 */
export interface LightingChangePayload {
  preset: string | null;
  /** null = 清除覆盖，回到「跟随场景」 */
  params: LightingParams | null;
  source: string;
  duration?: number;
}

export interface LightingState {
  /** Rust 侧是 camelCase（LightingState 结构体） */
  overrideParams: LightingParams | null;
  overridePreset: string | null;
  overrideSource: string;
  currentSceneId: string | null;
}

export async function listLightingPresets(): Promise<LightingPreset[]> {
  return invoke<LightingPreset[]>("lighting_list_presets");
}

export async function applyLighting(req: {
  preset?: string | null;
  params?: LightingParams | null;
}): Promise<LightingChangePayload> {
  return invoke<LightingChangePayload>("lighting_apply", { req });
}

export async function clearLighting(): Promise<void> {
  await invoke("lighting_clear");
}

export async function getLighting(): Promise<LightingState> {
  return invoke<LightingState>("lighting_get");
}

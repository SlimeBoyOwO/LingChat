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
  /** true = 用户自建（「我的预设」），可编辑可删除；内置预设不可改 */
  custom: boolean;
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
  /** 前端上报回来的「屏幕实际渲染」状态，后端只存不判 */
  activePreset: string | null;
  activeSource: string;
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

/**
 * 上报「屏幕上正在渲染的光影」给后端。
 *
 * 后端自己算不出来：设置面板选的全局预设存在 localStorage，渲染优先级又排在
 * 运行时覆盖之后。不回报的话 `lighting_get` 只能看见覆盖，模型会误判成
 * 「没打灯」或沿用上一次的结果，从而跳过用户要求的切换。
 */
export async function reportLightingActive(req: {
  preset?: string | null;
  source: string;
}): Promise<void> {
  await invoke("lighting_report_active", { req });
}

/** 保存自建预设的入参。`replaceId` 有值即覆盖同名自建预设（编辑后保存）。 */
export interface SaveLightingPresetReq {
  name: string;
  description?: string;
  mood?: string[];
  params: LightingParams;
  replaceId?: string | null;
}

export async function saveLightingPreset(req: SaveLightingPresetReq): Promise<LightingPreset> {
  return invoke<LightingPreset>("lighting_preset_save", {
    req: {
      name: req.name,
      description: req.description ?? "",
      mood: req.mood ?? [],
      params: req.params,
      replaceId: req.replaceId ?? null,
    },
  });
}

export async function deleteLightingPreset(id: string): Promise<void> {
  await invoke("lighting_preset_delete", { id });
}

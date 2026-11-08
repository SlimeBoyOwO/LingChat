import type {
  LightSource,
  LightingCameraParams,
  LightingCompositingParams,
  LightingGlobalParams,
  LightingMaterialParams,
  LightingShot,
} from "@/api/services/scene";

export type PresetCategory = "all" | "favorite" | "daily" | "atmosphere" | "drama" | "dark";

export interface LightingPreset {
  id: string;
  name: string;
  cat: Exclude<PresetCategory, "all" | "favorite">;
  desc: string;
  builtin: boolean;
  bgKey?: string;
  shotKey?: LightingShot;
  camera: LightingCameraParams;
  global: LightingGlobalParams;
  compositing: LightingCompositingParams;
  material: LightingMaterialParams;
  lights: LightSource[];
  _overridden?: boolean;
}

export const SHOT_CAMERA_MAP: Record<
  LightingShot,
  { name: string; zoom: number; fy: number; dof: number }
> = {
  fs: { name: "远景", zoom: 0.85, fy: 0.05, dof: 0.0 },
  ws: { name: "全景", zoom: 1.05, fy: 0.1, dof: 0.0 },
  ms: { name: "中景", zoom: 1.25, fy: 0.16, dof: 0.0 },
  cu: { name: "近景", zoom: 1.48, fy: 0.28, dof: 0.35 },
  ecu: { name: "特写", zoom: 1.78, fy: 0.38, dof: 0.75 },
};

export const DEFAULT_GLOBAL: LightingGlobalParams = {
  master: 1.0,
  ambient_blend: 0.15,
  albedo_protect: 0.3,
  soft_knee: 0.72,
  lift: 0.12,
  mono: 0.0,
};

export const DEFAULT_COMPOSITING: LightingCompositingParams = {
  light_wrap: 0.24,
  rim_width: 4.8,
  bloom_int: 0.16,
  bloom_thresh: 0.72,
};

export const DEFAULT_MATERIAL: LightingMaterialParams = {
  use_normal: true,
  normal_strength: 1.0,
  cel_thresh: 0.5,
  cel_softness: 0.06,
  spec_int: 0.45,
  spec_gloss: 24.0,
  self_shadow_int: 0.35,
  use_depth: true,
};

export const DEFAULT_CAMERA: LightingCameraParams = {
  shot: "ms",
  zoom: 1.25,
  focus_y: 0.16,
  dof: 0.0,
};

export const DEFAULT_NEW_LIGHT: Record<LightSource["type"], Omit<LightSource, "id" | "name">> = {
  point: {
    type: "point",
    enabled: true,
    is_key: false,
    color: "#fff8f0",
    intensity: 1.0,
    x: 0.5,
    y: 0.3,
    z: 0.55,
    radius: 0.85,
    softness: 0.65,
    azimuth: 95,
    elevation: 62,
    cone: 50,
    penumbra: 0.4,
  },
  directional: {
    type: "directional",
    enabled: true,
    is_key: false,
    color: "#f0f4ff",
    intensity: 0.35,
    x: 0.5,
    y: 0.5,
    z: 0.8,
    radius: 1.5,
    softness: 0.8,
    azimuth: 110,
    elevation: 45,
    cone: 180,
    penumbra: 0.0,
  },
  spot: {
    type: "spot",
    enabled: true,
    is_key: false,
    color: "#fffae8",
    intensity: 1.4,
    x: 0.5,
    y: 0.25,
    z: 0.6,
    radius: 0.95,
    softness: 0.5,
    azimuth: 80,
    elevation: 65,
    cone: 45,
    penumbra: 0.35,
  },
  ambient: {
    type: "ambient",
    enabled: true,
    is_key: false,
    color: "#e2e8f0",
    intensity: 0.3,
    x: 0.5,
    y: 0.5,
    z: 0.0,
    radius: 1.0,
    softness: 0.0,
    azimuth: 0,
    elevation: 90,
    cone: 180,
    penumbra: 0.0,
  },
};

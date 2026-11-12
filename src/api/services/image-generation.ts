import { invoke } from "@tauri-apps/api/core";

export interface ImageGenerationSettings {
  provider: string;
  model: string;
  base_url: string;
  api_key: string;
  models_url: string;
  size: string;
  quality: string;
  timeout_secs: number;
}
export interface ImageProviderInfo {
  id: string;
  label: string;
  custom: boolean;
}
export interface ImageModelCatalog {
  models: string[];
  source: string;
  warning: string;
}
export interface GeneratedImage {
  path: string;
  model: string;
  provider: string;
  scene: { id: string; scene_name: string };
}
export const listImageProviders = () =>
  invoke<ImageProviderInfo[]>("list_image_generation_providers");
export const listImageModels = (settings: ImageGenerationSettings, refresh = false) =>
  invoke<ImageModelCatalog>("list_image_generation_models", { settings, refresh });
export const saveImageSettings = (settings: ImageGenerationSettings) =>
  invoke<void>("save_image_generation_settings", { settings });
export const generateImage = (prompt: string, name: string, applyBackground = false) =>
  invoke<GeneratedImage>("generate_image", {
    request: { prompt, name, apply_background: applyBackground },
  });

import { invoke } from '@tauri-apps/api/core'

export interface FilterParams {
  brightness: number
  contrast: number
  saturation: number
  sepia: number
  glow_radius: number
  glow_color: string
}

export interface LightingParams {
  character: FilterParams
  background: FilterParams
  overlay_enabled: boolean
  blend_mode: string
  light_x: number
  light_y: number
  overlay_color1: string
  overlay_color2: string
  overlay_radius: number
  overlay_opacity: number
  overlay_target: string
}

export interface SceneInfo {
  id: string
  scene_name: string
  scene_description: string
  background: string | null
  lighting: LightingParams | null
  created_at: string
  updated_at: string
}

export interface CreateSceneRequest {
  scene_name: string
  scene_description: string
  background: string
  lighting?: LightingParams | null
}

export interface UpdateSceneRequest {
  id: string
  scene_name: string
  scene_description: string
  background: string
  lighting?: LightingParams | null
}

export async function listScenes(): Promise<SceneInfo[]> {
  return invoke<SceneInfo[]>('list_scenes')
}

export async function createScene(req: CreateSceneRequest): Promise<SceneInfo> {
  return invoke<SceneInfo>('create_scene', { req })
}

export async function updateScene(req: UpdateSceneRequest): Promise<SceneInfo> {
  return invoke<SceneInfo>('update_scene', { req })
}

export async function deleteScene(id: string): Promise<void> {
  return invoke('delete_scene', { id })
}

export async function selectScene(sceneId: string | null): Promise<void> {
  return invoke('select_scene', { sceneId })
}

export async function setSceneAwareness(enabled: boolean): Promise<void> {
  return invoke('set_scene_awareness', { enabled })
}

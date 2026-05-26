import { invoke } from '@tauri-apps/api/core'

export interface SceneInfo {
  id: string
  scene_name: string
  scene_description: string
  background: string | null
  created_at: string
  updated_at: string
}

export interface CreateSceneRequest {
  scene_name: string
  scene_description: string
  background: string
}

export interface UpdateSceneRequest {
  id: string
  scene_name: string
  scene_description: string
  background: string
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

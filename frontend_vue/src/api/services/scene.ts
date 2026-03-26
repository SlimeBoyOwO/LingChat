import http from '../http'

export interface SceneInfo {
  filename: string
  description: string
  preview?: string
}

export async function listScenes(): Promise<SceneInfo[]> {
  const response = await http.get<{ scenes: SceneInfo[] }>('/v1/chat/scene/list')
  return response.scenes
}

export async function loadScene(filename: string): Promise<void> {
  await http.post('/v1/chat/scene/load', { scene_filename: filename })
}

export async function clearScene(): Promise<void> {
  await http.post('/v1/chat/scene/clear')
}
export async function addScene(scene_filename: string, description: string): Promise<void> {
  await http.post('/v1/chat/scene/add', { scene_filename, description })
}

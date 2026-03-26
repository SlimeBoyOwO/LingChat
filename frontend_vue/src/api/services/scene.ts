import http from '../http'

export interface SceneInfo {
  sceneName: string
  sceneImage: string
  sceneDescription: string
}

export async function listScenes(): Promise<SceneInfo[]> {
  const response = await http.get<{ scenes: SceneInfo[] }>('/v1/chat/scene/list')
  return response.scenes
}

export async function saveScene(scene: SceneInfo): Promise<void> {
  await http.post('/v1/chat/scene/save', scene)
}

export async function deleteScene(sceneName: string): Promise<void> {
  await http.post('/v1/chat/scene/delete', { sceneName })
}

export async function loadScene(sceneName: string, immediate: boolean = false): Promise<void> {
  await http.post('/v1/chat/scene/load', { sceneName, immediate })
}

export async function clearScene(): Promise<void> {
  await http.post('/v1/chat/scene/clear')
}

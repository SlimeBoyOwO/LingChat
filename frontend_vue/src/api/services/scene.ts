import http from '../http'

export interface SceneInfo {
  id: string
  sceneName: string
  sceneImage: string | null
  sceneDescription: string
  imageUrl?: string
  createdAt: string
  updatedAt: string
  source?: string
}

export interface CreateSceneRequest {
  sceneName: string
  sceneImage: string | null
  sceneDescription: string
  autoAnalyze: boolean
}

// 列出所有场景
export async function listScenes(): Promise<SceneInfo[]> {
  const response = await http.get<{ scenes: SceneInfo[] }>('/v1/chat/scene/list')
  return response.scenes
}

// 加载场景
export async function loadScene(sceneId: string, triggerAIResponse: boolean): Promise<void> {
  await http.post('/v1/chat/scene/load', {
    sceneId,
    triggerAIResponse,
  })
}

// 清除场景
export async function clearScene(): Promise<void> {
  await http.post('/v1/chat/scene/clear')
}

// 创建场景
export async function createScene(request: CreateSceneRequest): Promise<SceneInfo> {
  const response = await http.post<{ scene: SceneInfo }>('/v1/chat/scene/create', request)
  return response.scene
}

// 更新场景
export async function updateScene(
  sceneId: string,
  updates: Partial<CreateSceneRequest>,
): Promise<SceneInfo> {
  const response = await http.put<{ scene: SceneInfo }>('/v1/chat/scene/update', {
    id: sceneId,
    ...updates,
  })
  return response.scene
}

// 删除场景
export async function deleteScene(sceneId: string): Promise<void> {
  await http.delete('/v1/chat/scene/delete', { data: { id: sceneId } })
}

// src/api/scene.ts
import axios from 'axios'

export interface SceneInfo {
  filename: string
  description: string
  preview?: string
}

export async function listScenes(): Promise<SceneInfo[]> {
  const response = await axios.get('/api/v1/chat/scene/list')
  return response.data.scenes
}

export async function loadScene(filename: string): Promise<void> {
  await axios.post('/api/v1/chat/scene/load', { scene_filename: filename })
}

export async function clearScene(): Promise<void> {
  await axios.post('/api/v1/chat/scene/clear')
}

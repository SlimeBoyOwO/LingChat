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

/** NovelAI 账号状态（测试连接用）。 */
export interface NovelaiSubscription {
  /** 订阅等级，3 及以上为 Opus */
  tier: number
  active: boolean
  /** 剩余 Anlas（免费额度内的生成不消耗它） */
  anlas: number
  is_opus: boolean
}

/**
 * 为指定场景生成背景图（手动触发，不弹确认框 —— 按下按钮本身就是同意）。
 * 与对话工具 scene_generate 共用同一把生成锁，同时只能有一张在生成。
 */
export async function generateSceneBackground(
  sceneId: string,
  promptTags: string,
): Promise<SceneInfo> {
  return invoke<SceneInfo>('generate_scene_background', { sceneId, promptTags })
}

/** 验证 NovelAI Token 是否可用。 */
export async function testNovelaiConnection(): Promise<NovelaiSubscription> {
  return invoke<NovelaiSubscription>('test_novelai_connection')
}

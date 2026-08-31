import { invoke } from '@tauri-apps/api/core'

export interface AdventureInfo {
  adventure_folder: string
  name: string
  description: string
  recommand_start: string
  order: number
  status: 'locked' | 'unlocked' | 'in_progress' | 'completed'
  unlocked_at?: string
  completed_at?: string
  unlock_conditions?: Array<{
    type: string
    threshold?: number
    start_hour?: number
    end_hour?: number
    adventure_folder?: string
    achievement_id?: string
  }>
}

export interface UnlockedAdventure {
  adventure_folder: string
  name: string
  description: string
  character_folder: string
  order: number
}

/**
 * 获取指定角色的所有羁绊冒险列表（含解锁状态）
 */
export const getCharacterAdventures = async (
  characterFolder: string,
): Promise<AdventureInfo[]> => {
  return invoke<AdventureInfo[]>('list_character_adventures', { characterFolder })
}

/**
 * 获取所有羁绊冒险（含解锁状态）
 */
export const getAllAdventures = async (): Promise<AdventureInfo[]> => {
  return invoke<AdventureInfo[]>('list_all_adventures')
}

/**
 * 启动指定羁绊冒险
 */
export const startAdventure = async (adventureFolder: string): Promise<void> => {
  return invoke('start_adventure', { adventureFolder })
}

/**
 * 手动检测是否有新冒险可解锁
 */
export const checkUnlocks = async (): Promise<UnlockedAdventure[]> => {
  return invoke<UnlockedAdventure[]>('check_adventure_unlocks')
}

/**
 * 重置冒险进度以供重玩
 */
export const resetAdventure = async (adventureFolder: string): Promise<void> => {
  return invoke('reset_adventure', { adventureFolder })
}

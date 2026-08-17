import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

/** plugins.json 条目（对应 Rust 侧 MarketPackage） */
export interface MarketPackage {
  id: string
  name: string
  type: string
  version: string
  author?: string | null
  description?: string | null
  download_url: string
  sha256?: string | null
  size?: number | null
  manifest?: Record<string, unknown> | null
  review_report_url?: string | null
}

/** 已安装记录（对应 Rust 侧 InstalledRecord） */
export interface InstalledRecord {
  id: string
  version: string
  type: string
  dir: string
}

/** 安装进度事件（market:progress） */
export interface MarketProgress {
  id: string
  phase: 'download' | 'install' | 'done'
  percent: number
  bytes?: number
}

export async function fetchMarketIndex(): Promise<MarketPackage[]> {
  return invoke('market_fetch_index')
}

export async function fetchInstalled(): Promise<InstalledRecord[]> {
  return invoke('market_installed')
}

export async function installPackage(id: string): Promise<void> {
  return invoke('market_install', { id })
}

export async function uninstallPackage(id: string): Promise<void> {
  return invoke('market_uninstall', { id })
}

export async function clearMarketCache(): Promise<void> {
  return invoke('market_clear_cache')
}

/** 订阅安装进度事件，返回取消函数 */
export function onMarketProgress(
  cb: (p: MarketProgress) => void,
): Promise<UnlistenFn> {
  return listen<MarketProgress>('market:progress', (event) => cb(event.payload))
}

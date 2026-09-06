import { invoke } from '@tauri-apps/api/core'

/** 一个已识别的 DLC 剧本包（standalone 下带 dlc.json 标记的目录） */
export interface DlcInfo {
  /** 目录名（standalone/<folder_key>） */
  folder_key: string
  /** story_config 的 script_name */
  name: string
  description?: string
  content_warning?: string
  version?: string
  author?: string
  imported_at?: string
}

/** 列出所有已识别的 DLC 剧本包 */
export const listDlcs = async (): Promise<DlcInfo[]> => {
  return await invoke<DlcInfo[]>('list_dlcs')
}

/** 导入一个 DLC 剧本包（zip），解压到 standalone 并立即注册，返回识别到的信息 */
export const importDlc = async (zipPath: string): Promise<DlcInfo> => {
  return await invoke<DlcInfo>('import_dlc', { zipPath })
}

/** 卸载一个 DLC（从引擎摘除并删除目录；仅限带 dlc.json 标记的包） */
export const removeDlc = async (folderKey: string): Promise<void> => {
  await invoke('remove_dlc', { folderKey })
}

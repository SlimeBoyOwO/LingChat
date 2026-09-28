import { invoke } from '@tauri-apps/api/core'

/**
 * 数据备份 / 恢复内容选择。
 */
export interface BackupSelections {
  database: boolean
  settings: boolean
  frontendPreferences: boolean
  characters: boolean
  backgrounds: boolean
  musics: boolean
  ambients: boolean
}

/**
 * 备份清单（manifest.json）。
 */
export interface BackupManifest {
  version: number
  exportedAt: number
  appVersion: string
  selections: {
    database: boolean
    settings: boolean
    frontendPreferences: boolean
    characters: boolean
    backgrounds: boolean
    musics: boolean
    ambients: boolean
  }
}

/**
 * 导入结果。
 */
export interface ImportResult {
  frontendPreferencesJson: string | null
  databaseImported: boolean
  settingsImported: boolean
  filesRestored: string[]
  /** 是否需要重启应用以加载新数据 */
  needsRestart: boolean
}

/**
 * 全选默认值。
 */
export const ALL_SELECTIONS: BackupSelections = {
  database: true,
  settings: true,
  frontendPreferences: true,
  characters: true,
  backgrounds: true,
  musics: true,
  ambients: true,
}

/**
 * 导出数据备份。
 *
 * @param selections  要包含的内容
 * @param frontendPreferencesJson  Pinia persist 序列化的 JSON 字符串
 * @param destPath    保存路径（桌面文件路径 或 Android SAF content:// URI）
 */
export async function exportDataBackup(
  selections: BackupSelections,
  frontendPreferencesJson: string | null,
  destPath: string,
): Promise<void> {
  await invoke('export_data_backup', {
    selections,
    frontendPreferencesJson,
    destPath,
  })
}

/**
 * 读取备份文件清单（预览，不恢复）。
 */
export async function peekDataBackup(srcPath: string): Promise<BackupManifest> {
  return invoke<BackupManifest>('peek_data_backup', { srcPath })
}

/**
 * 从备份文件恢复数据。
 *
 * @param srcPath     备份文件路径
 * @param selections  要恢复的内容
 */
export async function importDataBackup(
  srcPath: string,
  selections: BackupSelections,
): Promise<ImportResult> {
  return invoke<ImportResult>('import_data_backup', {
    srcPath,
    selections,
  })
}

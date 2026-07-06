import { ref } from 'vue'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { invoke } from '@tauri-apps/api/core'

// ─── 类型定义 ────────────────────────────────────────────────

export interface ResourceSyncInfo {
  available: boolean
  newVersion: number
  currentVersion: number
  filesToAdd: SyncFileEntry[]
  filesToModify: SyncFileEntry[]
  totalSize: number
}

export interface SyncFileEntry {
  path: string
  sha256: string
  size: number
  changeType: 'add' | 'modify'
}

export interface ResourceSyncResult {
  success: boolean
  filesSynced: number
  message: string
}

export type UpdatePhase =
  | 'idle'
  | 'checking'
  | 'app-update-available'
  | 'complete'
  | 'error'

// ─── 共享状态 ────────────────────────────────────────────────

const phase = ref<UpdatePhase>('idle')
const appVersion = ref('')
const appReleaseNotes = ref('')
const errorMessage = ref('')

// 资源同步独立状态
const resourceSyncInfo = ref<ResourceSyncInfo | null>(null)
const resourceSyncPhase = ref<'idle' | 'review' | 'syncing' | 'complete' | 'error'>('idle')
const resourceSyncError = ref('')

// ─── 导出 composable ─────────────────────────────────────────

export function useUpdater() {
  /** 检查 app 更新，返回是否有可用更新 */
  async function checkForUpdates(): Promise<boolean> {
    phase.value = 'checking'
    errorMessage.value = ''

    try {
      const update = await check()
      if (update?.available) {
        appVersion.value = update.version ?? ''
        appReleaseNotes.value = update.body ?? ''
        phase.value = 'app-update-available'
        return true
      }
      // 无更新
      phase.value = 'idle'
      return false
    } catch (e) {
      const msg = String(e)
      console.error('[Updater] App update check failed:', msg)
      phase.value = 'error'
      // 提取有意义的部分（去掉冗长的 URL 和底层错误堆栈）
      if (msg.includes('error sending request')) {
        errorMessage.value = '网络连接失败，无法访问更新服务器。请检查网络后重试。'
      } else {
        errorMessage.value = msg.length > 200 ? msg.slice(0, 200) : msg
      }
      return false
    }
  }

  /** 安装 app 更新并重启 */
  async function installAppUpdate(): Promise<void> {
    try {
      const update = await check()
      if (update?.available) {
        await update.downloadAndInstall()
        await relaunch()
      }
    } catch (e) {
      console.error('[Updater] App update install failed:', e)
      phase.value = 'error'
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 稍后提醒 — 关闭对话框 */
  function remindLater() {
    phase.value = 'idle'
  }

  /** 重置状态 */
  function reset() {
    phase.value = 'idle'
    errorMessage.value = ''
  }

  // ─── 资源同步 ──────────────────────────────────────────────

  /** 检查数据资源更新（对比 .official manifest） */
  async function checkResourceSync(): Promise<boolean> {
    try {
      const info = await invoke<ResourceSyncInfo>('check_resource_sync')
      resourceSyncInfo.value = info
      if (info.available) {
        resourceSyncPhase.value = 'review'
        return true
      }
      return false
    } catch (e) {
      resourceSyncPhase.value = 'error'
      resourceSyncError.value = String(e)
      return false
    }
  }

  /** 应用选中的资源文件同步 */
  async function applyResourceSync(selectedFiles: string[]): Promise<void> {
    resourceSyncPhase.value = 'syncing'
    try {
      const result = await invoke<ResourceSyncResult>('apply_resource_sync', {
        selectedFiles,
      })
      if (result.success) {
        resourceSyncPhase.value = 'complete'
      } else {
        resourceSyncPhase.value = 'error'
        resourceSyncError.value = result.message
      }
    } catch (e) {
      resourceSyncPhase.value = 'error'
      resourceSyncError.value = String(e)
    }
  }

  /** 获取本地数据版本号 */
  async function getDataVersion(): Promise<number> {
    try {
      return await invoke<number>('get_data_version')
    } catch {
      return 0
    }
  }

  /** 重置资源同步状态 */
  function resetResourceSync() {
    resourceSyncInfo.value = null
    resourceSyncPhase.value = 'idle'
    resourceSyncError.value = ''
  }

  return {
    // App 更新
    phase,
    appVersion,
    appReleaseNotes,
    errorMessage,
    checkForUpdates,
    installAppUpdate,
    remindLater,
    reset,
    // 资源同步
    resourceSyncInfo,
    resourceSyncPhase,
    resourceSyncError,
    checkResourceSync,
    applyResourceSync,
    getDataVersion,
    resetResourceSync,
  }
}

import { ref } from 'vue'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// ─── 类型定义 ────────────────────────────────────────────────

export interface DataUpdateInfo {
  available: boolean
  newVersion: number
  currentVersion: number
  filesToAdd: string[]
  filesToModify: string[]
  filesToRemove: string[]
  totalDownloadSize: number
}

export interface DataUpdateProgress {
  phase?: string
  progress?: number
  current?: number
  total?: number
  currentFile?: string
  message?: string
}

export interface DataUpdateResult {
  success: boolean
  message: string
  newVersion: number
}

export type UpdatePhase =
  | 'idle'
  | 'checking'
  | 'app-update-available'
  | 'data-update-available'
  | 'downloading-data'
  | 'extracting-data'
  | 'applying-data'
  | 'complete'
  | 'error'

// ─── 共享状态 ────────────────────────────────────────────────

const phase = ref<UpdatePhase>('idle')
const appVersion = ref('')
const appReleaseNotes = ref('')
const dataInfo = ref<DataUpdateInfo | null>(null)
const dataProgress = ref<DataUpdateProgress>({})
const errorMessage = ref('')

// ─── 内部监听 ────────────────────────────────────────────────

let unlistenProgress: (() => void) | null = null
let unlistenComplete: (() => void) | null = null
let initCount = 0

async function setupEventListeners() {
  if (unlistenProgress) return
  unlistenProgress = await listen<DataUpdateProgress>(
    'data-update-progress',
    (event) => {
      dataProgress.value = event.payload
      if (event.payload.phase === 'downloading') {
        phase.value = 'downloading-data'
      } else if (event.payload.phase === 'extracting') {
        phase.value = 'extracting-data'
      } else if (event.payload.phase === 'applying') {
        phase.value = 'applying-data'
      }
    },
  )
  unlistenComplete = await listen<DataUpdateResult>(
    'data-update-complete',
    (event) => {
      const result = event.payload
      if (result.success) {
        phase.value = 'complete'
      } else {
        phase.value = 'error'
        errorMessage.value = result.message
      }
    },
  )
}

function teardownEventListeners() {
  if (unlistenProgress) {
    unlistenProgress()
    unlistenProgress = null
  }
  if (unlistenComplete) {
    unlistenComplete()
    unlistenComplete = null
  }
}

// ─── 导出 composable ─────────────────────────────────────────

export function useUpdater() {
  function init() {
    if (initCount++ > 0) return
    setupEventListeners()
  }

  function destroy() {
    if (--initCount > 0) return
    teardownEventListeners()
  }

  /** 检查所有更新（app + data），返回是否有可用更新 */
  async function checkForUpdates(): Promise<boolean> {
    phase.value = 'checking'
    errorMessage.value = ''

    let hasAppUpdate = false
    let hasDataUpdate = false

    // 1. 检查 app 更新 (tauri-plugin-updater)
    try {
      const update = await check()
      if (update?.available) {
        hasAppUpdate = true
        appVersion.value = update.version ?? ''
        appReleaseNotes.value = update.body ?? ''
        phase.value = 'app-update-available'
      }
    } catch (e) {
      console.warn('[Updater] App update check failed:', e)
      // app 更新检查失败不阻塞 data 更新检查
    }

    // 2. 检查 data 更新 (自定义 Rust 模块)
    try {
      const info = await invoke<DataUpdateInfo>('check_data_update')
      dataInfo.value = info
      if (info.available) {
        hasDataUpdate = true
        if (!hasAppUpdate) {
          phase.value = 'data-update-available'
        }
      }
    } catch (e) {
      console.warn('[Updater] Data update check failed:', e)
    }

    if (!hasAppUpdate && !hasDataUpdate) {
      phase.value = 'idle'
    }

    return hasAppUpdate || hasDataUpdate
  }

  /** 安装 app 更新并重启 */
  async function installAppUpdate(): Promise<void> {
    try {
      const update = await check()
      if (update?.available) {
        await update.downloadAndInstall()
        // 重启应用以应用更新
        await relaunch()
      }
    } catch (e) {
      console.error('[Updater] App update install failed:', e)
      phase.value = 'error'
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 执行 data 更新 */
  async function applyDataUpdate(): Promise<DataUpdateResult> {
    try {
      phase.value = 'downloading-data'
      errorMessage.value = ''

      const result = await invoke<DataUpdateResult>('apply_data_update')
      return result
    } catch (e) {
      console.error('[Updater] Data update failed:', e)
      phase.value = 'error'
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 按正确顺序执行所有更新：先 app，再 data */
  async function installAllUpdates(): Promise<void> {
    const hasAppUpdate = phase.value === 'app-update-available' || appVersion.value !== ''

    if (hasAppUpdate) {
      // 安装 app 更新（内部会调用 relaunch）
      await installAppUpdate()
      // installAppUpdate 会调用 relaunch()，通常下面不会执行
    }

    // 如果 app 没有更新（或 relaunch 后不会到这一步），执行 data 更新
    const hasDataUpdate = dataInfo.value?.available
    if (hasDataUpdate) {
      await applyDataUpdate()
      phase.value = 'complete'
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
    dataInfo.value = null
    dataProgress.value = {}
  }

  return {
    // 状态
    phase,
    appVersion,
    appReleaseNotes,
    dataInfo,
    dataProgress,
    errorMessage,
    // 方法
    init,
    destroy,
    checkForUpdates,
    installAppUpdate,
    applyDataUpdate,
    installAllUpdates,
    remindLater,
    reset,
  }
}

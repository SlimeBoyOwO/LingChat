import { onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import { useRoleArchiveStore } from '@/stores/modules/ui/role-archive'
import {
  importRoleFromPath,
  exportRoleToPath,
  cancelRoleImport,
  rescanRoles,
  type ArchiveFormat,
  type ConflictPolicy,
  type ImportResult,
  type ExportResult,
  type EntryEvent,
  type RoleImportStartedEvent,
} from '@/api/services/role-archive'

// 名称超过 28 个字符时从左侧截断，并添加省略号。
function truncateName(name: string, max = 28): string {
  if (name.length <= max) return name
  return '\u2026' + name.slice(name.length - max + 1)
}

function detectFormat(fileName: string): ArchiveFormat | null {
  const lower = fileName.toLowerCase()
  if (lower.endsWith('.zip')) return 'zip'
  if (lower.endsWith('.7z')) return '7z'
  return null
}

function isAndroidContentUri(p: string): boolean {
  return p.startsWith('content://')
}

// 模块级单例监听器：应用生命周期内只注册一次。
let progressUnlisten: UnlistenFn | null = null
let errorUnlisten: UnlistenFn | null = null
let startedUnlisten: UnlistenFn | null = null
let progressTimer: number | null = null
let listenersInitialized = false
// 当前正在进行的导入任务 id；cancel() 时传给后端以找到正确的取消令牌。
// 后端有全局导入并发锁，所以同一时刻最多只有一个任务。
let currentTaskId: string | null = null

function clearTimers() {
  if (progressTimer !== null) {
    window.clearInterval(progressTimer)
    progressTimer = null
  }
}

async function ensureListeners() {
  if (listenersInitialized) return
  listenersInitialized = true
  const store = useRoleArchiveStore()
  progressUnlisten = await listen<EntryEvent>('role:import-progress', (event) => {
    const evt = event.payload
    if (evt.phase === 'entry') {
      if (evt.bytes_total > 0) {
        const pct = Math.min(90, Math.floor((evt.bytes_done / evt.bytes_total) * 90))
        store.import.percent = pct
      }
      store.import.message = truncateName(evt.name)
    } else if (evt.phase === 'finished') {
      store.import.percent = 100
    }
  })
  errorUnlisten = await listen<string>('role:import-error', (event) => {
    store.import.phase = 'error'
    store.import.error = event.payload || 'import failed'
    clearTimers()
  })
  startedUnlisten = await listen<RoleImportStartedEvent>('role:import-started', (event) => {
    // 后端刚生成 task_id 时立刻发送，前端存下来给 cancel() 用。
    currentTaskId = event.payload?.task_id ?? null
  })
}

export function useRoleImportExport() {
  const store = useRoleArchiveStore()

  async function setupListeners() {
    await ensureListeners()
  }

  // 使用基于耗时的指数曲线模拟进度，最高推进到 90%。
  function startFakeProgress() {
    store.import.percent = 0
    const start = Date.now()
    clearTimers()
    progressTimer = window.setInterval(() => {
      const elapsed = Date.now() - start
      const pct = Math.min(90, Math.floor(90 * (1 - Math.exp(-elapsed / 3000))))
      store.import.percent = pct
      if (pct >= 90) {
        store.import.message = '\u5b8c\u6210\u4e2d'
      }
    }, 200)
  }

  async function pickAndImport(conflict: ConflictPolicy = 'rename') {
    console.log('[RoleArchive] pickAndImport \u5f00\u59cb, conflict=', conflict)
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'Archive', extensions: ['zip', '7z'] }],
    })
    if (!selected) {
      console.log('[RoleArchive] pickAndImport \u7528\u6237\u53d6\u6d88\u9009\u62e9')
      return
    }
    const filePath = typeof selected === 'string' ? selected : (selected as any).path
    if (!filePath) return
    const fileName = filePath.split(/[\\/]/).pop() || filePath
    const format = detectFormat(fileName)
    if (!format) {
      console.warn('[RoleArchive] pickAndImport \u4e0d\u652f\u6301\u7684\u683c\u5f0f:', fileName)
      store.import.phase = 'error'
      store.import.error = '\u4ec5\u652f\u6301 .zip / .7z \u683c\u5f0f'
      return
    }
    await runImport(filePath, fileName, format, conflict)
  }

  async function runImport(
    filePath: string,
    fileName: string,
    format: ArchiveFormat,
    conflict: ConflictPolicy,
  ) {
    store.resetImport()
    store.import.phase = 'running'
    store.import.fileName = truncateName(fileName)
    store.import.format = format
    store.import.conflict = conflict
    store.import.startedAt = Date.now()
    console.log(
      '[RoleArchive] runImport \u5f00\u59cb: file=%s, format=%s, conflict=%s, androidUri=%s',
      fileName, format, conflict, isAndroidContentUri(filePath),
    )
    store.import.percent = -1
    // 每次 runImport 都先把 task_id 清空，等后端 emit role:import-started 后再回填。
    currentTaskId = null
    await setupListeners()

    try {
      console.log(
        '[RoleArchive] backend path import: source=%s, androidSaf=%s',
        filePath,
        isAndroidContentUri(filePath),
      )
      startFakeProgress()
      const result: ImportResult = await importRoleFromPath({
        path: filePath,
        format,
        conflict,
        fileName,
      })

      store.import.result = result
      store.import.phase = 'done'
      store.import.percent = 100
      store.import.message = `\u5bfc\u5165\u6210\u529f: ${result.role_name}`
      console.log(
        '[RoleArchive] runImport \u5b8c\u6210: role_name=%s, role_id=%s, action=%s, bytes=%d',
        result.role_name, result.role_id, result.conflict_action, result.bytes_extracted,
      )
      clearTimers()
    } catch (e: any) {
      console.error('[RoleArchive] runImport \u5931\u8d25:', e)
      store.import.phase = 'error'
      store.import.error = typeof e === 'string' ? e : e?.message || String(e)
      clearTimers()
    } finally {
      currentTaskId = null
      clearTimers()
    }
  }

  async function cancel() {
    console.log('[RoleArchive] cancel \u53d1\u9001\u53d6\u6d88\u8bf7\u6c42', { taskId: currentTaskId })
    if (!currentTaskId) {
      // 极端情况：用户在 task_id 还没回填时（或者根本没在导入）就点了取消。
      console.warn('[RoleArchive] cancel 没有可用的 task_id，跳过后端调用')
    } else {
      try {
        await cancelRoleImport(currentTaskId)
      } catch (e) {
        console.warn('[RoleArchive] cancel \u540e\u7aef\u8c03\u7528\u5931\u8d25:', e)
      }
    }
    store.import.phase = 'cancelled'
    store.import.message = '\u5df2\u53d6\u6d88'
    clearTimers()
    clearTimers()
  }

  async function doExport(roleId: number, roleName: string, format: ArchiveFormat) {
    console.log('[RoleArchive] doExport \u5f00\u59cb: roleId=%d, roleName=%s, format=%s', roleId, roleName, format)
    store.resetExport()
    store.export.phase = 'running'
    store.export.roleName = roleName
    store.export.format = format
    store.export.percent = -1
    store.export.message = '\u7b49\u5f85\u4fdd\u5b58\u4f4d\u7f6e...'

    // 提前生成建议文件名，规则与后端的名称清洗和时间戳逻辑保持一致。
    const safeName = (roleName || 'role').replace(/[\\/:*?"<>|]/g, '_').trim() || 'role'
    const ts = Date.now()
    const suggestedName = `${safeName}_${ts}.${format}`

    let savedPath: string | null = null
    try {
      savedPath = await saveDialog({
        defaultPath: suggestedName,
        filters: [{ name: format === '7z' ? '7Z' : 'ZIP', extensions: [format] }],
      })
      if (!savedPath) {
        console.log('[RoleArchive] doExport \u7528\u6237\u53d6\u6d88\u4fdd\u5b58')
        store.export.phase = 'cancelled'
        store.export.message = '\u5df2\u53d6\u6d88'
        return
      }
      console.log('[RoleArchive] doExport \u7528\u6237\u9009\u62e9: %s, \u5f00\u59cb\u538b\u7f29+\u590d\u5236', savedPath)
      store.export.message = '\u6b63\u5728\u538b\u7f29...'
      store.export.percent = -1

      // 桌面端使用原生文件系统复制，Android SAF 由后端通过 android-fs 写入。
      const res: ExportResult = await exportRoleToPath({
        roleId,
        format,
        destPath: savedPath,
      })
      console.log('[RoleArchive] doExport backend wrote destination: %s', res.temp_path)

      store.export.phase = 'done'
      store.export.savedPath = savedPath
      store.export.percent = 100
      store.export.message = '\u5bfc\u51fa\u6210\u529f'
      console.log('[RoleArchive] doExport \u5b8c\u6210: dest=%s, size=%dB (%dMB)', savedPath, res.size_bytes, Math.floor(res.size_bytes / 1024 / 1024))
    } catch (e: any) {
      console.error('[RoleArchive] doExport \u5931\u8d25:', e)
      store.export.phase = 'error'
      store.export.error = typeof e === 'string' ? e : e?.message || String(e)
    }
  }

  async function rescan() {
    console.log('[RoleArchive] rescan \u8c03\u7528')
    try {
      const ids = await rescanRoles()
      console.log('[RoleArchive] rescan \u5b8c\u6210: %d \u4e2a\u89d2\u8272', ids.length)
      return ids
    } catch (e) {
      console.error('[RoleArchive] rescan \u5931\u8d25:', e)
      throw e
    }
  }

  onUnmounted(() => {
    clearTimers()
  })

  return {
    store,
    pickAndImport,
    runImport,
    cancel,
    doExport,
    rescan,
  }
}

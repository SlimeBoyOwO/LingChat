import { onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import { readFile } from '@tauri-apps/plugin-fs'
import { useRoleArchiveStore } from '@/stores/modules/ui/role-archive'
import {
  importRole,
  importRoleFromPath,
  exportRoleToPath,
  cancelRoleImport,
  rescanRoles,
  type ArchiveFormat,
  type ConflictPolicy,
  type ImportResult,
  type ExportResult,
  type EntryEvent,
} from '@/api/services/role-archive'

const LARGE_FILE_THRESHOLD = 50 * 1024 * 1024 // 50MB
const PROGRESS_FILE_THRESHOLD = 5 * 1024 * 1024 // 5MB

// 28-char truncation with leading ellipsis
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

// Module-level singleton: listeners registered once for app lifetime
let progressUnlisten: UnlistenFn | null = null
let errorUnlisten: UnlistenFn | null = null
let progressTimer: number | null = null
let listenersInitialized = false

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
}

export function useRoleImportExport() {
  const store = useRoleArchiveStore()

  async function setupListeners() {
    await ensureListeners()
  }

  // Elapsed-time exponential curve to 90% for files >= 5MB (indeterminate mode)
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
    await setupListeners()

    try {
      let result: ImportResult

      if (isAndroidContentUri(filePath)) {
        // Android SAF: read bytes via plugin-fs, then single-invoke importRole
        console.log('[RoleArchive] Android SAF \u8bfb\u53d6 bytes:', filePath)
        const bytes = await readFile(filePath)
        store.import.sizeBytes = bytes.byteLength
        console.log('[RoleArchive] Android \u8bfb\u53d6\u5b8c\u6210: %dB (%dMB)', bytes.byteLength, Math.floor(bytes.byteLength / 1024 / 1024))
        if (bytes.byteLength >= PROGRESS_FILE_THRESHOLD) {
          startFakeProgress()
        }
        if (bytes.byteLength > LARGE_FILE_THRESHOLD) {
          // Exceeds single-invoke limit; backend will reject.
          store.import.phase = 'error'
          store.import.error = `\u6587\u4ef6 ${Math.floor(bytes.byteLength / 1024 / 1024)}MB \u8d85\u8fc7\u5355\u6b21\u4e0a\u4f20 50MB \u9650\u5236`
          clearTimers()
          clearTimers()
          return
        }
        result = await importRole({
          bytes: new Uint8Array(bytes),
          format,
          conflict,
          fileName,
        })
      } else {
        // Desktop: pass path directly, backend handles stat + size check + extraction
        console.log('[RoleArchive] Desktop \u8d70 importRoleFromPath:', filePath)
        startFakeProgress()
        result = await importRoleFromPath({ path: filePath, format, conflict, fileName })
      }

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
      clearTimers()
    }
  }

  async function cancel() {
    console.log('[RoleArchive] cancel \u53d1\u9001\u53d6\u6d88\u8bf7\u6c42')
    try {
      await cancelRoleImport()
    } catch (e) {
      console.warn('[RoleArchive] cancel \u540e\u7aef\u8c03\u7528\u5931\u8d25:', e)
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

    // Generate suggested filename up front (mirrors backend sanitize + timestamp)
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

      // Single backend invoke: compress to temp + std::fs::copy to dest + cleanup temp.
      // Bypasses Tauri fs scope (std::fs not subject to plugin-fs permissions).
      const res: ExportResult = await exportRoleToPath({
        roleId,
        format,
        destPath: savedPath,
      })

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

import { save as saveDialog } from '@tauri-apps/plugin-dialog'
import { useRoleArchiveStore } from '@/stores/modules/ui/role-archive'
import { createArchiveImportController } from '@/composables/useArchiveImport'
import {
  importRoleFromPath,
  exportRoleToPath,
  cancelRoleImport,
  rescanRoles,
  type ArchiveFormat,
  type ConflictPolicy,
  type ImportResult,
  type ExportResult,
} from '@/api/services/role-archive'

// 导入流程（选文件 / 进度事件 / 取消 / 结果提示）由 useArchiveImport 的工厂承载，
// 这里只绑定角色领域的三元组：事件前缀 + store + 后端命令。
const archiveImport = createArchiveImportController({
  eventPrefix: 'role',
  errorEvent: 'role:import-error',
  logTag: 'RoleArchive',
  defaultConflict: 'rename',
  getStore: useRoleArchiveStore,
  invoke: (args) =>
    importRoleFromPath({
      path: args.path,
      format: args.format,
      conflict: (args.conflict as ConflictPolicy) ?? 'rename',
      fileName: args.fileName,
    }),
  cancelTask: cancelRoleImport,
  successMessage: (result) => `导入成功: ${(result as ImportResult).role_name}`,
})

export function useRoleImportExport() {
  const store = useRoleArchiveStore()

  async function doExport(roleId: number, roleName: string, format: ArchiveFormat) {
    console.log('[RoleArchive] doExport 开始: roleId=%d, roleName=%s, format=%s', roleId, roleName, format)
    store.resetExport()
    store.export.phase = 'running'
    store.export.roleName = roleName
    store.export.format = format
    store.export.percent = -1
    store.export.message = '等待保存位置...'

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
        console.log('[RoleArchive] doExport 用户取消保存')
        store.export.phase = 'cancelled'
        store.export.message = '已取消'
        return
      }
      console.log('[RoleArchive] doExport 用户选择: %s, 开始压缩+复制', savedPath)
      store.export.message = '正在压缩...'
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
      store.export.message = '导出成功'
      console.log('[RoleArchive] doExport 完成: dest=%s, size=%dB (%dMB)', savedPath, res.size_bytes, Math.floor(res.size_bytes / 1024 / 1024))
    } catch (e: any) {
      console.error('[RoleArchive] doExport 失败:', e)
      store.export.phase = 'error'
      store.export.error = typeof e === 'string' ? e : e?.message || String(e)
    }
  }

  async function rescan() {
    console.log('[RoleArchive] rescan 调用')
    try {
      const ids = await rescanRoles()
      console.log('[RoleArchive] rescan 完成: %d 个角色', ids.length)
      return ids
    } catch (e) {
      console.error('[RoleArchive] rescan 失败:', e)
      throw e
    }
  }

  return {
    store,
    setupListeners: archiveImport.setupListeners,
    pickAndImport: (conflict: ConflictPolicy = 'rename') => archiveImport.pickAndImport(conflict),
    runImport: (
      filePath: string,
      fileName: string,
      format: ArchiveFormat | undefined,
      conflict: ConflictPolicy,
    ) => archiveImport.runImport(filePath, fileName, format, conflict),
    cancel: archiveImport.cancel,
    doExport,
    rescan,
  }
}

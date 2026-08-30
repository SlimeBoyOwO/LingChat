/**
 * 插件压缩包导入的前端流程。
 *
 * 与角色导入共用 `createArchiveImportController`，只替换事件前缀、store 与后端命令。
 * 进度条由 `ImportProgressBar source="plugin"` 渲染。
 */
import { usePluginArchiveStore } from '@/stores/modules/ui/plugin-archive'
import { createArchiveImportController } from '@/composables/useArchiveImport'
import {
  importPluginFromPath,
  cancelPluginImport,
  type PluginConflictPolicy,
  type PluginImportResult,
} from '@/api/services/plugins'

const archiveImport = createArchiveImportController({
  eventPrefix: 'plugin',
  logTag: 'PluginArchive',
  defaultConflict: 'overwrite',
  getStore: usePluginArchiveStore,
  // 后端以 manifest.id 命名插件目录，压缩包文件名只用于进度条标题，故不向下传递。
  invoke: (args) =>
    importPluginFromPath({
      path: args.path,
      format: args.format,
      conflict: (args.conflict as PluginConflictPolicy) ?? 'overwrite',
    }),
  cancelTask: cancelPluginImport,
  successMessage: (result) => {
    const r = result as PluginImportResult
    return `导入成功: ${r.plugin_name ?? r.plugin_id}`
  },
})

export function usePluginImport() {
  const store = usePluginArchiveStore()

  return {
    store,
    pickAndImport: (conflict: PluginConflictPolicy = 'overwrite') =>
      archiveImport.pickAndImport(conflict),
    cancel: archiveImport.cancel,
  }
}

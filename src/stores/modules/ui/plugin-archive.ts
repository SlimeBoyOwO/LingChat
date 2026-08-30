import { defineStore } from 'pinia'
import { initialImportSlice, type ArchiveImportSlice } from './archive-import'

export type { ArchiveImportSlice, ImportPhase } from './archive-import'

/**
 * 插件压缩包导入状态。
 * 与角色导入分开一份实例，但共用同一个后端并发锁：同一时刻全局只有一个解压任务。
 */
export const usePluginArchiveStore = defineStore('plugin-archive', {
  state: () => ({
    import: initialImportSlice('overwrite') as ArchiveImportSlice,
  }),

  actions: {
    resetImport() {
      this.import = initialImportSlice('overwrite')
    },
  },
})

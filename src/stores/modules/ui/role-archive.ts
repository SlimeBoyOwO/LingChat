import { defineStore } from 'pinia'
import type { ImportResult, ConflictPolicy, ArchiveFormat } from '@/api/services/role-archive'

export type ImportPhase = 'idle' | 'running' | 'done' | 'error' | 'cancelled'

export interface ImportState {
  phase: ImportPhase
  fileName: string
  format: ArchiveFormat
  conflict: ConflictPolicy
  // 0-100, -1 = indeterminate
  percent: number
  message: string
  result: ImportResult | null
  error: string
  startedAt: number
  sizeBytes: number
}

export interface ExportState {
  phase: ImportPhase
  roleName: string
  format: ArchiveFormat
  percent: number
  message: string
  savedPath: string
  error: string
}

const initialImport = (): ImportState => ({
  phase: 'idle',
  fileName: '',
  format: 'zip',
  conflict: 'rename',
  percent: -1,
  message: '',
  result: null,
  error: '',
  startedAt: 0,
  sizeBytes: 0,
})

const initialExport = (): ExportState => ({
  phase: 'idle',
  roleName: '',
  format: 'zip',
  percent: -1,
  message: '',
  savedPath: '',
  error: '',
})

export const useRoleArchiveStore = defineStore('role-archive', {
  state: () => ({
    import: initialImport(),
    export: initialExport(),
  }),

  actions: {
    resetImport() {
      this.import = initialImport()
    },
    resetExport() {
      this.export = initialExport()
    },
  },
})

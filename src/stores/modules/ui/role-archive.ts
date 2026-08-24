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

export type NoticePhase = 'idle' | 'active'

export interface CorrectedNotice {
  phase: NoticePhase
  title: string
  message: string
  durationMs: number
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

const initialCorrected = (): CorrectedNotice => ({
  phase: 'idle',
  title: '',
  message: '',
  durationMs: 5000,
})

export const useRoleArchiveStore = defineStore('role-archive', {
  state: () => ({
    import: initialImport(),
    export: initialExport(),
    corrected: initialCorrected(),
  }),

  actions: {
    resetImport() {
      this.import = initialImport()
    },
    resetExport() {
      this.export = initialExport()
    },
    showCorrected(payload: { title: string; message: string; durationMs?: number }) {
      this.corrected.phase = 'active'
      this.corrected.title = payload.title
      this.corrected.message = payload.message
      this.corrected.durationMs = payload.durationMs ?? 5000
    },
    dismissCorrected() {
      this.corrected = initialCorrected()
    },
  },
})

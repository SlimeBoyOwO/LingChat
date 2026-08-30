import { defineStore } from 'pinia'
import type { ArchiveFormat, ConflictPolicy } from '@/api/services/role-archive'
import {
  initialImportSlice,
  type ArchiveImportSlice,
  type ImportPhase,
} from './archive-import'

export type { ImportPhase }
export type ImportState = ArchiveImportSlice

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

/** 角色压缩包导入/导出状态；导入分片与插件导入共用 `ArchiveImportSlice` 形状。 */
export const useRoleArchiveStore = defineStore('role-archive', {
  state: () => ({
    import: initialImportSlice('rename') as ArchiveImportSlice,
    export: initialExport(),
    corrected: initialCorrected(),
  }),

  actions: {
    resetImport() {
      this.import = initialImportSlice('rename')
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

export type { ArchiveFormat, ConflictPolicy }

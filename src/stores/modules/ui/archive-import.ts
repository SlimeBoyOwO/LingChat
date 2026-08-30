import type { ArchiveFormat, ArchiveImportResult } from '@/api/services/archive'

export type ImportPhase = 'idle' | 'running' | 'done' | 'error' | 'cancelled'

/**
 * 压缩包导入进度条读取的状态分片，角色与插件共用同一形状。
 * 各领域的 store 只保留自己的一份实例（见 `role-archive.ts` / `plugin-archive.ts`）。
 */
export interface ArchiveImportSlice {
  phase: ImportPhase
  fileName: string
  format: ArchiveFormat
  /** 领域相关的冲突策略取值，故为 string（角色 rename/skip/overwrite，插件 overwrite/abort）。 */
  conflict: string
  /** 0-100，-1 = indeterminate */
  percent: number
  message: string
  result: ArchiveImportResult | null
  error: string
  startedAt: number
  sizeBytes: number
}

export const initialImportSlice = (conflict = 'rename'): ArchiveImportSlice => ({
  phase: 'idle',
  fileName: '',
  format: 'zip',
  conflict,
  percent: -1,
  message: '',
  result: null,
  error: '',
  startedAt: 0,
  sizeBytes: 0,
})

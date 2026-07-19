import { invoke } from '@tauri-apps/api/core'

export type ArchiveFormat = 'zip' | '7z'
export type ConflictPolicy = 'rename' | 'skip' | 'overwrite'

export interface ImportResult {
  role_id: number | null
  role_name: string
  conflict_action: string
  warnings: string[]
  bytes_extracted: number
}

export interface ExportResult {
  temp_path: string
  suggested_name: string
  size_bytes: number
}

export interface EntryEvent {
  phase: 'started' | 'entry' | 'finished' | 'error'
  index: number
  total: number
  name: string
  bytes_done: number
  bytes_total: number
  bytes_entry: number
}

// Small file (< 50MB): pass bytes directly via single invoke
export async function importRole(params: {
  bytes: number[] | Uint8Array
  format: ArchiveFormat
  conflict: ConflictPolicy
  fileName?: string
}): Promise<ImportResult> {
  const bytes = params.bytes instanceof Uint8Array ? Array.from(params.bytes) : params.bytes
  return invoke<ImportResult>('import_role', {
    bytes,
    format: params.format,
    conflict: params.conflict,
    fileName: params.fileName ?? null,
  })
}

// Large file (> 50MB): pass file:// URI or absolute path
export async function importRoleFromPath(params: {
  path: string
  format: ArchiveFormat
  conflict: ConflictPolicy
  fileName?: string
}): Promise<ImportResult> {
  return invoke<ImportResult>('import_role_from_path', {
    path: params.path,
    format: params.format,
    conflict: params.conflict,
    fileName: params.fileName ?? null,
  })
}

export async function cancelRoleImport(): Promise<void> {
  await invoke('cancel_role_import')
}

export async function exportRole(params: {
  roleId: number
  format: ArchiveFormat
}): Promise<ExportResult> {
  return invoke<ExportResult>('export_role', {
    roleId: params.roleId,
    format: params.format,
  })
}

// Export and write to the user-chosen destination in one backend invoke.
// Desktop paths use std::fs; Android content URIs use android-fs SAF support.
export async function exportRoleToPath(params: {
  roleId: number
  format: ArchiveFormat
  destPath: string
}): Promise<ExportResult> {
  return invoke<ExportResult>('export_role_to_path', {
    roleId: params.roleId,
    format: params.format,
    destPath: params.destPath,
  })
}

export async function rescanRoles(): Promise<number[]> {
  return invoke<number[]>('rescan_roles')
}

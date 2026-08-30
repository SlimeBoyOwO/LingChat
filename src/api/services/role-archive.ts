import { invoke } from '@tauri-apps/api/core'
import type { ArchiveImportResult, ArchiveFormat, ConflictPolicy } from './archive'

export type {
  ArchiveFormat,
  ConflictPolicy,
  EntryEvent,
  ImportStartedEvent,
} from './archive'

export interface ImportResult extends ArchiveImportResult {
  role_id: number | null
  role_name: string
}

export interface ExportResult {
  temp_path: string
  suggested_name: string
  size_bytes: number
}

// 保留字节导入接口，供已经在内存中持有压缩包数据的调用方使用。
export async function importRole(params: {
  bytes: number[] | Uint8Array
  format?: ArchiveFormat  // 可选；后端用 magic 裁决
  conflict: ConflictPolicy
  fileName?: string
}): Promise<ImportResult> {
  const bytes = params.bytes instanceof Uint8Array ? Array.from(params.bytes) : params.bytes
  return invoke<ImportResult>('import_role', {
    bytes,
    format: params.format ?? null,
    conflict: params.conflict,
    fileName: params.fileName ?? null,
  })
}

// 推荐的导入接口：支持桌面文件路径和 Android SAF 内容 URI。
export async function importRoleFromPath(params: {
  path: string
  format?: ArchiveFormat  // 可选；后端用 magic 裁决
  conflict: ConflictPolicy
  fileName?: string
}): Promise<ImportResult> {
  return invoke<ImportResult>('import_role_from_path', {
    path: params.path,
    format: params.format ?? null,
    conflict: params.conflict,
    fileName: params.fileName ?? null,
  })
}

// 后端取消接口需要 task_id；前端通过监听 role:import-started 事件拿到当前任务的 id。
export async function cancelRoleImport(taskId: string): Promise<void> {
  await invoke('cancel_role_import', { taskId })
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

// 通过一次后端调用完成压缩，并写入用户选择的目标位置。
// 桌面端使用原生文件系统，Android 内容 URI 使用 android-fs SAF 接口。
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

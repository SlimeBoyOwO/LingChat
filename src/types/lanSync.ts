// ─── LAN 同步相关类型 ────────────────────────────────────────

/** 局域网中发现的对等设备 */
export interface PeerInfo {
  deviceId: string
  deviceName: string
  host: string
  port: number
  dataVersion: number
  fileCount: number
}

/** 同步文件操作 */
export interface SyncFileOp {
  path: string
  sha256: string
  size: number
  reason: 'new' | 'modified' | 'newer'
}

/** 同步计划 */
export interface SyncPlan {
  direction: 'pull' | 'push'
  peer: PeerInfo
  filesToTransfer: SyncFileOp[]
  filesToDelete: string[]
  totalBytes: number
}

/** 同步进度事件 */
export interface SyncProgressEvent {
  phase: 'scanning' | 'comparing' | 'transferring' | 'complete'
  current: number
  total: number
  progress: number // 0-100
  currentFile: string | null
  bytesTransferred: number
  message: string | null
}

/** 同步完成结果 */
export interface SyncResult {
  success: boolean
  direction: string
  filesDownloaded: number
  filesDeleted: number
  /** 因文件被锁定而暂存的数量（重启后生效） */
  filesStaged: number
  bytesTransferred: number
  message: string
}

/** 同步阶段 */
export type SyncPhase =
  | 'idle'
  | 'scanning'
  | 'fetching'
  | 'planning'
  | 'executing'
  | 'complete'
  | 'error'

/** 对话框视图 */
export type DialogView = 'device-list' | 'sync-plan' | 'progress' | 'result'

export interface DownloadProgress {
  asset_id: string
  bytes_done: number
  total_bytes: number
  percent: number
}

export type ProgressListener = (progress: DownloadProgress) => void

export interface ProgressBus {
  subscribe(listener: ProgressListener): () => void
  dispatch(progress: DownloadProgress): void
  readonly listenerCount: number
}

export function createProgressBus(): ProgressBus {
  const listeners = new Set<ProgressListener>()
  return {
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    dispatch(progress) {
      for (const listener of listeners) listener(progress)
    },
    get listenerCount() {
      return listeners.size
    },
  }
}

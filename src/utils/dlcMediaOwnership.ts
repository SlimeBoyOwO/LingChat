export interface DlcMediaReleaseDetail {
  folderKey: string
}

/** 判断 Tauri asset URL/Windows 路径是否位于指定 standalone DLC 目录内。 */
export function isOwnedByStandaloneDlc(value: string | undefined, folderKey: string): boolean {
  if (!value || value === 'None' || !folderKey) return false
  let normalized = value.replace(/\\/g, '/')
  try {
    normalized = decodeURIComponent(normalized)
  } catch {
    // 非 URL 字符串继续按原值做保守路径比较。
  }
  normalized = normalized.replace(/\\/g, '/').toLowerCase()
  return normalized.includes(`/standalone/${folderKey.toLowerCase()}/`)
}

export function releaseFolderFromEvent(event: Event): string {
  return ((event as CustomEvent<DlcMediaReleaseDetail>).detail?.folderKey ?? '').trim()
}

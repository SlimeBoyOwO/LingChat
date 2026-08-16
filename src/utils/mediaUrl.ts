/**
 * 媒体文件播放 URL 转换工具
 *
 * - 桌面端：直接返回 asset 协议 URL（WebView2 能正确处理 206 分块流）
 * - Android：asset 协议每个响应最多 1MB（tauri 源码硬编码 MAX_LEN=1000*1024，
 *   无法配置）。安卓媒体栈对 open-ended Range（bytes=N-）的截断响应不会
 *   继续请求后续分块——大文件「播一会就停」（消耗完前两个 1MB 块后卡住），
 *   OGG 探测（需要文件末尾页）直接失败。因此改为整文件 fetch
 *   （fetch 不带 Range 头时 asset 协议返回完整 200）→ blob URL，缓存复用。
 */
import { convertFileSrc } from '@tauri-apps/api/core'
import { isAndroid } from '@/utils/platform'

// 原始路径 → blob URL 缓存（同会话内复用，避免每次播放都重新读取）
const blobCache = new Map<string, string>()

/** 把原始文件路径（或已转换的 asset/blob/data/http URL）转成可播放 URL */
export async function toPlayableMediaUrl(path: string): Promise<string> {
  if (
    path.startsWith('blob:') ||
    path.startsWith('data:') ||
    path.startsWith('http://') ||
    path.startsWith('https://') ||
    path.startsWith('asset:')
  ) {
    return path
  }
  if (!isAndroid()) return convertFileSrc(path)

  const cached = blobCache.get(path)
  if (cached) return cached

  const resp = await fetch(convertFileSrc(path))
  if (!resp.ok) throw new Error(`媒体加载失败(${resp.status}): ${path}`)
  const blob = await resp.blob()
  const url = URL.createObjectURL(blob)
  blobCache.set(path, url)
  return url
}

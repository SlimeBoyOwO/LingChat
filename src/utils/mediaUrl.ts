/**
 * 媒体文件播放 URL 转换工具
 *
 * - 桌面端：直接返回 asset 协议 URL（WebView2 能正确处理 206 分块流）
 * - Android：asset 协议每个响应最多 1MB（tauri 源码硬编码 MAX_LEN=1000*1024，
 *   无法配置）。安卓媒体栈对 open-ended Range（bytes=N-）的截断响应不会
 *   继续请求后续分块——大文件「播一会就停」（消耗完前两个 1MB 块后卡住），
 *   OGG 探测（需要文件末尾页）直接失败。因此改为整文件 fetch
 *   （fetch 不带 Range 头时 asset 协议返回完整 200）→ blob URL，缓存复用。
 *
 * 缓存语义：同会话内按路径复用 blob URL（文件被删除/替换后同路径仍播放
 * 旧内容，重启会话即刷新）；达到上限时按最早插入的顺序淘汰并释放 blob。
 */
import { convertFileSrc } from '@tauri-apps/api/core'
import { isAndroid } from '@/utils/platform'

// 原始路径 → blob URL（Promise 形态，天然去重并发请求）
const blobCache = new Map<string, Promise<string>>()

/** 缓存上限：防会话内无限增长（几十首歌 × 数 MB 在合理范围内） */
const MAX_CACHE_ENTRIES = 30

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

  const pending = (async () => {
    const resp = await fetch(convertFileSrc(path))
    if (!resp.ok) throw new Error(`媒体加载失败(${resp.status}): ${path}`)
    const blob = await resp.blob()
    return URL.createObjectURL(blob)
  })()
  blobCache.set(path, pending)
  // 加载失败时移出缓存，允许下次重试
  pending.catch(() => blobCache.delete(path))

  // LRU 上限：淘汰最早插入的条目，待其 resolve 后释放 blob
  if (blobCache.size > MAX_CACHE_ENTRIES) {
    const oldestPath = blobCache.keys().next().value
    if (oldestPath) {
      const oldest = blobCache.get(oldestPath)
      blobCache.delete(oldestPath)
      if (oldest) void oldest.then((url) => URL.revokeObjectURL(url))
    }
  }
  return pending
}

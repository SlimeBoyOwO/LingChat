/**
 * 媒体文件播放 URL 转换工具
 *
 * - 桌面端：asset 协议直连（WebView2 能正确处理 206 分块流）
 * - Android：asset 协议每个 206 响应上限 1MB（tauri 硬编码，不可配置），
 *   安卓媒体栈对截断的 open-ended Range 响应不再请求后续分块——
 *   大文件播一会即停、OGG 探测（需文件末尾页）直接失败。
 *   故整文件 fetch（无 Range → 完整 200）→ blob URL，缓存复用。
 *
 * 缓存语义：同会话按路径复用（删后重导同名文件仍播放旧内容，重启会话刷新）；
 * 超上限按插入顺序淘汰并释放 blob。
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import { isAndroid } from "@/utils/platform";

// 原始路径 → blob URL（Promise 形态，天然去重并发请求）
const blobCache = new Map<string, Promise<string>>();

/** 缓存上限：防会话内无限增长 */
const MAX_CACHE_ENTRIES = 30;

/** 把原始文件路径（或已转换的 asset/blob/data/http URL）转成可播放 URL */
export async function toPlayableMediaUrl(path: string): Promise<string> {
  if (
    path.startsWith("blob:") ||
    path.startsWith("data:") ||
    path.startsWith("http://") ||
    path.startsWith("https://") ||
    path.startsWith("asset:")
  ) {
    return path;
  }
  if (!isAndroid()) return convertFileSrc(path);

  const cached = blobCache.get(path);
  if (cached) return cached;

  const pending = (async () => {
    const resp = await fetch(convertFileSrc(path));
    if (!resp.ok) throw new Error(`媒体加载失败(${resp.status}): ${path}`);
    const blob = await resp.blob();
    return URL.createObjectURL(blob);
  })();
  blobCache.set(path, pending);
  // 加载失败时移出缓存，允许下次重试
  pending.catch(() => blobCache.delete(path));

  // 超限淘汰最早的条目，待其 resolve 后释放 blob
  if (blobCache.size > MAX_CACHE_ENTRIES) {
    const oldestPath = blobCache.keys().next().value;
    if (oldestPath) {
      const oldest = blobCache.get(oldestPath);
      blobCache.delete(oldestPath);
      if (oldest) void oldest.then((url) => URL.revokeObjectURL(url));
    }
  }
  return pending;
}

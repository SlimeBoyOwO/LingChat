/**
 * 媒体播放 URL 转换：桌面端走 asset 协议；Android 的 asset 协议单个 206 响应上限 1MB，
 * 大文件会中途停、OGG 探测失败，故整文件 fetch 成 blob URL 并按路径缓存复用。
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import { isAndroid } from "@/utils/platform";

// 路径 → blob URL（Promise 形态可去重并发请求）
const blobCache = new Map<string, Promise<string>>();

/** 缓存上限。 */
const MAX_CACHE_ENTRIES = 30;

/** 转为可播放 URL。 */
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
  // 失败时移出缓存，允许重试
  pending.catch(() => blobCache.delete(path));

  // 超限淘汰最早条目，resolve 后释放 blob
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

let currentAudio: HTMLAudioElement | null = null;

/** 播放本地音频；同一时刻只保留一个。 */
export async function playLocalAudio(path: string): Promise<HTMLAudioElement> {
  stopCurrentAudio();
  const url = await toPlayableMediaUrl(path);
  const audio = new Audio(url);
  currentAudio = audio;
  audio.onended = () => {
    if (currentAudio === audio) currentAudio = null;
  };
  audio.onerror = () => {
    if (currentAudio === audio) currentAudio = null;
  };
  audio.play().catch(() => {
    if (currentAudio === audio) currentAudio = null;
  });
  return audio;
}

/** 停止当前播放。 */
export function stopCurrentAudio(): void {
  if (currentAudio) {
    const el = currentAudio;
    currentAudio = null;
    try {
      el.pause();
    } catch {
      /* 忽略 */
    }
  }
}

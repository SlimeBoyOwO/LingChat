/**
 * 从文件路径或 content:// URI 中提取文件名并解码 URL 编码。
 *
 * - 取路径最后一段（兼容 `/` 与 `\` 分隔符）
 * - 去掉 query 参数（`?xxx`）
 * - 仅对包含 `%XX` 序列的字符串做 decode（避免误伤普通文件名里的 `%` 字符）；
 *   decode 遇到非法序列时兜底返回原值
 *
 * 桌面端路径、Android SAF content URI、convertFileSrc 生成的 URL 均可处理。
 */
export function decodePathFileName(path: string): string {
  const last = path.split(/[\\/]/).pop() || path
  const withoutQuery = last.split('?')[0]
  if (!/%[0-9A-Fa-f]{2}/.test(withoutQuery)) return withoutQuery
  try {
    return decodeURIComponent(withoutQuery)
  } catch {
    return withoutQuery
  }
}

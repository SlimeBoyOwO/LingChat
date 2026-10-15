/**
 * 从文件路径、URL 或 Android SAF content URI 中提取可读的文件名。
 *
 * Android content URI 的末段是「文档 id」，常见形态是 percent-encoded 的
 * path，例如 `primary%3AMusic%2F%E6%AD%8C.mp3`（`%3A`=`:`、`%2F`=`/`、
 * 中文按 UTF-8 逐字节编码）。因此必须 **先解码、再按分隔符取最后一段**：
 * 只做其中任意一步都会留下 `primary%3AMusic%2F…` 或 `primary:Music/…`
 * 这种前缀（旧实现只取末段再解码，于是把整个 `primary:Music/…` 显示出来）。
 *
 * - 去掉 query 参数（`?xxx`）——在解码前处理，避免 `%3F` 解出的 `?` 被误删
 * - 宽容百分号解码：`%XX` 还原为字节后按 UTF-8 解码；孤立的 `%`（后面不是
 *   两位十六进制）原样保留。不像 `decodeURIComponent` 那样，因为个别非法
 *   序列就整段放弃解码（那样会退回显示 `%E6%AD%8C` 这类乱码）
 * - 循环解码到稳定（上限 3 轮）：个别链路会把 URI 再编码一次
 *   （`primary%253A…` → 解码一次仍是 `primary%3A…`，看着仍是乱码）
 * - 每轮都重新取末段：`%2F` 解出后会变成真正的路径分隔符
 * - 兼容 `/` 与 `\` 分隔符；桌面端路径、convertFileSrc URL 均可处理
 */
export function decodePathFileName(path: string): string {
  let name = lastPathSegment(path.split("?")[0]);
  for (let i = 0; i < 3; i++) {
    // 仅当真的含 `%XX` 才解码，避免误伤普通文件名里的 `%`（如「100%.mp3」）
    const decoded = /%[0-9A-Fa-f]{2}/.test(name) ? decodePercent(name) : name;
    if (decoded === name) break;
    name = lastPathSegment(decoded) || decoded;
  }
  return name;
}

/** 取路径最后一段（兼容 `/` 与 `\`）。 */
function lastPathSegment(s: string): string {
  return s.split(/[\\/]/).pop() || s;
}

/**
 * 宽容的百分号解码（RFC 3986 字节序列 → UTF-8）。
 * 非法 UTF-8 字节以 U+FFFD 取代，孤立的 `%` 原样保留，绝不抛异常。
 */
function decodePercent(s: string): string {
  const bytes: number[] = [];
  let i = 0;
  while (i < s.length) {
    if (s[i] === "%") {
      const hex = s.slice(i + 1, i + 3);
      if (/^[0-9A-Fa-f]{2}$/.test(hex)) {
        bytes.push(parseInt(hex, 16));
        i += 3;
        continue;
      }
      // 孤立的 `%`：按字面量保留
      bytes.push(0x25);
      i += 1;
      continue;
    }
    // 非 `%` 片段整体按 UTF-8 展开（正确处理非 ASCII 字符与代理对）
    const next = s.indexOf("%", i);
    const end = next === -1 ? s.length : next;
    for (const b of new TextEncoder().encode(s.slice(i, end))) bytes.push(b);
    i = end;
  }
  return new TextDecoder("utf-8", { fatal: false }).decode(new Uint8Array(bytes));
}

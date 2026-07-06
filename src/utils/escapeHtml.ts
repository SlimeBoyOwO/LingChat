/**
 * 转义 HTML 特殊字符，防止 innerHTML 注入。
 * 游戏脚本文本可能包含 <, >, &, ", ' 等字符。
 */
export function escapeHtml(text: string): string {
  const map: Record<string, string> = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#039;',
  }
  return text.replace(/[&<>"']/g, (ch) => map[ch] || ch)
}

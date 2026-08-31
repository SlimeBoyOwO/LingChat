/**
 * 颜色工具函数
 */

/**
 * 将十六进制颜色转换为 rgba 格式
 * @param hex - 十六进制颜色值（如 '#000e27'）
 * @param alpha - 透明度（0-1）
 * @returns rgba 字符串（如 'rgba(0, 14, 39, 0.7)'）
 */
export function hexToRgba(hex: string, alpha: number): string {
  const m = hex.replace('#', '').match(/^([0-9a-fA-F]{6})$/)
  if (!m) return `rgba(0,14,39,${alpha})`
  const r = parseInt(m[1]!.substring(0, 2), 16)
  const g = parseInt(m[1]!.substring(2, 4), 16)
  const b = parseInt(m[1]!.substring(4, 6), 16)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

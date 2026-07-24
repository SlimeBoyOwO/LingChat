import { invoke } from '@tauri-apps/api/core'

export interface FontFamilyInfo {
  name: string
}

// 字体族名列表（多为中文/英文族名）。一次 app 运行内只枚举一次，字体列表极少变化。
let cached: string[] | null = null

/**
 * 列出系统已安装的字体族名。
 * - Windows：Rust 侧用 GDI EnumFontFamiliesExW 真实枚举本机全部字体族。
 * - 其他平台：暂返回空数组（前端将仅显示“软件默认”项，不报错）。
 * @param force 强制重新枚举（忽略内存缓存）
 */
export async function listSystemFonts(force = false): Promise<string[]> {
  if (cached != null && !force) return cached
  try {
    const list = await invoke<FontFamilyInfo[]>('list_system_fonts')
    cached = list
      .map((f) => f.name)
      // 中文优先（按拼音/笔画排序不稳定，用 localeCompare 兼顾中英，分组见调用方）
      .sort((a, b) => a.localeCompare(b, 'zh-CN'))
  } catch (error: any) {
    console.error('枚举系统字体失败:', error)
    cached = [] // 失败则空，前端兜底显示“软件默认”
  }
  return cached
}
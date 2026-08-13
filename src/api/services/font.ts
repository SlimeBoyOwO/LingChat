import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'

export interface FontFamilyInfo {
  name: string
}

export interface ImportedFontInfo {
  name: string
  file_name: string
  file_path: string
}

// 系统字体族名缓存。一次 app 运行内只枚举一次。
let cached: string[] | null = null
// 导入字体缓存。一次 app 运行内只枚举一次。
let importedCache: ImportedFontInfo[] | null = null

/**
 * 列出系统已安装的字体族名。
 * - Windows：Rust 侧用 GDI EnumFontFamiliesExW 真实枚举本机全部字体族。
 * - 其他平台：暂返回空数组（前端将仅显示"软件默认"项，不报错）。
 */
export async function listSystemFonts(force = false): Promise<string[]> {
  if (cached != null && !force) return cached
  try {
    const list = await invoke<FontFamilyInfo[]>('list_system_fonts')
    cached = list
      .map((f) => f.name)
      .sort((a, b) => a.localeCompare(b, 'zh-CN'))
  } catch (error: any) {
    console.error('枚举系统字体失败:', error)
    cached = []
  }
  return cached
}

// ========== 导入字体管理 ==========

/** 导入用户选择的字体文件到 data/fonts/。 */
export async function importFont(path: string): Promise<ImportedFontInfo> {
  return invoke<ImportedFontInfo>('import_font', { path })
}

/** 列出 data/fonts/ 中所有已导入字体。 */
export async function listImportedFonts(): Promise<ImportedFontInfo[]> {
  return invoke<ImportedFontInfo[]>('list_imported_fonts')
}

/**
 * 删除一个已导入字体。
 * 注意：`name` 参数应为带扩展名的文件名（即 `ImportedFontInfo.file_name`），
 * 而非去扩展名的 `name`。
 */
export async function deleteImportedFont(name: string): Promise<void> {
  await invoke('delete_imported_font', { name })
}

/** 带缓存的获取导入字体列表。 */
export async function getImportedFonts(force = false): Promise<ImportedFontInfo[]> {
  if (importedCache != null && !force) return importedCache
  try {
    importedCache = await listImportedFonts()
  } catch (e) {
    console.error('获取导入字体列表失败:', e)
    importedCache = []
  }
  return importedCache
}

/** 清空导入字体缓存（导入/删除后调用）。 */
export function clearImportedFontsCache(): void {
  importedCache = null
}

// 已注入的 @font-face style 标签 ID 集合
const fontFaceStyleIds = new Set<string>()

/**
 * 为指定字体文件注册 @font-face CSS 规则，使 webview 可使用该字体。
 * 同名字体幂等——会先移除旧规则再注入新规则。
 */
export function registerFontFace(name: string, filePath: string): void {
  const url = convertFileSrc(filePath)
  const styleId = `font-face-${CSS.escape(name)}`

  const existing = document.getElementById(styleId)
  if (existing) existing.remove()

  const style = document.createElement('style')
  style.id = styleId
  style.textContent = `
    @font-face {
      font-family: '${name}';
      src: url('${url}');
      font-display: swap;
    }
  `
  document.head.appendChild(style)
  fontFaceStyleIds.add(styleId)
}

/**
 * 为所有已导入字体批量注册 @font-face（应用启动时调用）。
 */
export function registerAllImportedFonts(fonts: ImportedFontInfo[]): void {
  for (const f of fonts) {
    registerFontFace(f.name, f.file_path)
  }
}

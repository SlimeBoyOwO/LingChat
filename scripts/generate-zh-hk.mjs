/**
 * 由简体中文语言包生成繁体中文（香港）语言包。
 *
 * 用法: node scripts/generate-zh-hk.mjs
 *
 * 原理：遍历 src/locales/zh-CN/ 下的每个命名空间文件，用 OpenCC（cn → hk）
 * 仅转换「字符串值」，键名（含中文键，如后端配置树分类名）保持原样——
 * 这些中文键需要与后端数据/调用方匹配，不能翻译。
 * 输出到 src/locales/zh-HK/，结构与原文件一致。
 *
 * 修改简体词条后重新运行本脚本即可同步繁体包。
 */
import { readdirSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const OpenCC = require('opencc-js')
const convert = OpenCC.Converter({ from: 'cn', to: 'hk' })

const __dirname = dirname(fileURLToPath(import.meta.url))
const srcDir = join(__dirname, '..', 'src', 'locales', 'zh-CN')
const outDir = join(__dirname, '..', 'src', 'locales', 'zh-HK')
mkdirSync(outDir, { recursive: true })

/** 递归转换对象中的字符串值（键名保持不变） */
function convertValues(obj) {
  if (typeof obj === 'string') return convert(obj)
  if (Array.isArray(obj)) return obj.map(convertValues)
  if (obj && typeof obj === 'object') {
    const out = {}
    for (const [k, v] of Object.entries(obj)) out[k] = convertValues(v)
    return out
  }
  return obj
}

/** 读取 `export default {...}` 的纯数据 TS 文件并求值 */
function loadDefaultExport(path) {
  const text = readFileSync(path, 'utf-8').replace(/^\uFEFF/, '')
  const body = text.replace(/^export\s+default\s+/, '').replace(/;?\s*$/, '')
  // 语言文件为纯对象字面量（无函数/引用），可安全求值
  return eval(`(${body})`)
}

let totalKeys = 0
function countStrings(obj) {
  if (typeof obj === 'string') return 1
  if (Array.isArray(obj)) return obj.reduce((s, v) => s + countStrings(v), 0)
  if (obj && typeof obj === 'object')
    return Object.values(obj).reduce((s, v) => s + countStrings(v), 0)
  return 0
}

for (const file of readdirSync(srcDir)) {
  if (!file.endsWith('.ts') || file === 'index.ts') continue
  const data = loadDefaultExport(join(srcDir, file))
  const converted = convertValues(data)
  const n = countStrings(converted)
  totalKeys += n
  const out =
    '// 本文件由 scripts/generate-zh-hk.mjs 自动生成（源：zh-CN/' +
    file +
    '），请勿手改\n' +
    'export default ' +
    JSON.stringify(converted, null, 2) +
    '\n'
  writeFileSync(join(outDir, file), out, 'utf-8')
  console.log(`✅ ${file}: ${n} 条`)
}

// 生成聚合 index.ts（结构对齐 zh-CN/index.ts，命名空间固定）
const namespaces = readdirSync(srcDir)
  .filter((f) => f.endsWith('.ts') && f !== 'index.ts')
  .map((f) => f.replace(/\.ts$/, ''))
const imports = namespaces.map((ns) => `import ${ns} from './${ns}'`).join('\n')
const body = namespaces.map((ns) => `  ${ns},`).join('\n')
writeFileSync(
  join(outDir, 'index.ts'),
  `// 本文件由 scripts/generate-zh-hk.mjs 自动生成，请勿手改\n${imports}\n\nexport default {\n${body}\n}\n`,
  'utf-8',
)
console.log(`📄 index.ts 已生成，共 ${totalKeys} 条词条`)

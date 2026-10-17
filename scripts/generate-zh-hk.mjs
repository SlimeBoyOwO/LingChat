/**
 * 同步繁体中文（香港）语言包。
 *
 * 用法:
 *   node scripts/generate-zh-hk.mjs          # 只补 zh-CN 新增的词条（默认，安全）
 *   node scripts/generate-zh-hk.mjs --force  # 用 OpenCC 结果整体覆盖（会丢掉人工译文）
 *
 * 原理：遍历 src/locales/zh-CN/ 下的每个命名空间文件，用 OpenCC（cn → hk）
 * 仅转换「字符串值」，键名（含中文键，如后端配置树分类名）保持原样——
 * 这些中文键需要与后端数据/调用方匹配，不能翻译。
 * 输出到 src/locales/zh-HK/，结构与原文件一致。
 *
 * ⚠️ zh-HK 的现有词条是**人工润色过的粤语文本**，与 OpenCC 的机械转换结果不同
 * （如 close「閂埋」、loading「載入緊…」）。所以默认只做「补缺」：保留已有译文，
 * 若 zh-CN 新增了词条才用 OpenCC 转换结果填进去。只有确认要放弃人工润色时才用
 * `--force`。
 */
import { readdirSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { basename, join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

import prettier from "prettier";

const require = createRequire(import.meta.url);
const OpenCC = require("opencc-js");
const ts = require("typescript");
const convert = OpenCC.Converter({ from: "cn", to: "hk" });

const __dirname = dirname(fileURLToPath(import.meta.url));
const srcDir = join(__dirname, "..", "src", "locales", "zh-CN");
const outDir = join(__dirname, "..", "src", "locales", "zh-HK");
mkdirSync(outDir, { recursive: true });

/** 递归转换对象中的字符串值（键名保持不变） */
function convertValues(obj) {
  if (typeof obj === "string") return convert(obj);
  if (Array.isArray(obj)) return obj.map(convertValues);
  if (obj && typeof obj === "object") {
    const out = {};
    for (const [k, v] of Object.entries(obj)) out[k] = convertValues(v);
    return out;
  }
  return obj;
}

/**
 * 读取 `export default {...}` 的纯数据 TS 文件。
 *
 * 用 TypeScript 编译器解析 AST，而不是 `eval` 字符串：
 * - 源文件允许带头部注释（`scriptEditor.ts` 就有一整段 `/** ... *\/`），
 *   旧实现用 `/^export\s+default\s+/` 剥前缀，注释一在就剥不掉，`eval` 里
 *   直接抛 `SyntaxError: Unexpected token 'export'`，整个生成流程中断，
 *   排在后面的 `settings/stores/ui/views` 全部生成不出来。
 * - `eval` 只能靠正则猜「哪里是数据」，而 AST 里值就是值。
 *
 * 只接受纯数据字面量（字符串/数字/布尔/null/数组/对象）：语言包里出现函数、
 * 引用或插值模板一律报错——那说明该文件不适合被本脚本翻译。
 */
function loadDefaultExport(path) {
  const text = readFileSync(path, "utf-8").replace(/^\uFEFF/, "");
  const source = ts.createSourceFile(path, text, ts.ScriptTarget.Latest);
  const exported = source.statements.find((stmt) => ts.isExportAssignment(stmt));
  if (!exported) {
    throw new Error(`${basename(path)}: 没找到 \`export default\``);
  }
  return literalToValue(exported.expression, basename(path));
}

/** TS 字面量节点 → 普通 JS 值；不支持的一律报错，不静默丢词条。 */
function literalToValue(node, where) {
  switch (node.kind) {
    case ts.SyntaxKind.StringLiteral:
    case ts.SyntaxKind.NoSubstitutionTemplateLiteral:
      return node.text;
    case ts.SyntaxKind.NumericLiteral:
      return Number(node.text);
    case ts.SyntaxKind.TrueKeyword:
      return true;
    case ts.SyntaxKind.FalseKeyword:
      return false;
    case ts.SyntaxKind.NullKeyword:
      return null;
    case ts.SyntaxKind.PrefixUnaryExpression:
      // 负数（如 `-1`）
      if (node.operator === ts.SyntaxKind.MinusToken && ts.isNumericLiteral(node.operand)) {
        return -Number(node.operand.text);
      }
      break;
    case ts.SyntaxKind.ParenthesizedExpression:
    case ts.SyntaxKind.AsExpression:
    case ts.SyntaxKind.SatisfiesExpression:
      // `( {...} )` / `... as const` / `... satisfies T`：剥掉外层继续
      return literalToValue(node.expression, where);
    case ts.SyntaxKind.ArrayLiteralExpression:
      return node.elements.map((el, i) => literalToValue(el, `${where}[${i}]`));
    case ts.SyntaxKind.ObjectLiteralExpression:
      return objectLiteralToValue(node, where);
  }
  throw new Error(`${where}: 语言文件只能是纯数据字面量，这里出现了 ${ts.SyntaxKind[node.kind]}`);
}

/** 对象字面量节点 → 普通对象（键名若是字符串字面量则取其文本）。 */
function objectLiteralToValue(node, where) {
  const out = {};
  for (const prop of node.properties) {
    if (!ts.isPropertyAssignment(prop)) {
      throw new Error(
        `${where}: 只支持普通属性，这里出现了 ${ts.SyntaxKind[prop.kind]}（展开/方法/简写属性）`,
      );
    }
    const key = propertyKey(prop.name, where);
    out[key] = literalToValue(prop.initializer, `${where}.${key}`);
  }
  return out;
}

/** 属性名 → 字符串键。 */
function propertyKey(name, where) {
  if (
    ts.isIdentifier(name) ||
    ts.isStringLiteral(name) ||
    ts.isNumericLiteral(name) ||
    ts.isNoSubstitutionTemplateLiteral(name)
  ) {
    return name.text;
  }
  throw new Error(`${where}: 不支持的键类型 ${ts.SyntaxKind[name.kind]}`);
}

let totalKeys = 0;
function countStrings(obj) {
  if (typeof obj === "string") return 1;
  if (Array.isArray(obj)) return obj.reduce((s, v) => s + countStrings(v), 0);
  if (obj && typeof obj === "object")
    return Object.values(obj).reduce((s, v) => s + countStrings(v), 0);
  return 0;
}

/** 读取现有 zh-HK 文件：返回 `{ header, value }`，文件不存在或解析失败返回 null。 */
function readExistingLocale(dest) {
  let text;
  try {
    text = readFileSync(dest, "utf-8");
  } catch {
    return null;
  }
  // 保留原文件的头部注释（人工维护的说明），合并模式下不要改写它。
  // 头部既可能是 `// ...`，也可能是 `/** ... */`（scriptEditor.ts 就是后者）。
  const head = /^[ \t]*export\s+default/m.exec(text);
  const header = head ? text.slice(0, head.index).trimEnd() : "";
  try {
    return { header, value: loadDefaultExport(dest) };
  } catch {
    return null;
  }
}

/**
 * 深合并：保留 zh-HK 里已有的译文（人工润色的粤语），只补 zh-CN 新增的键。
 * 删除的键（zh-CN 里已不存在）直接丢掉，与「以 zh-CN 为准」的结构保持一致。
 */
function mergeMissing(existing, generated, where, stats) {
  const merged = {};
  for (const [key, genValue] of Object.entries(generated)) {
    const here = `${where}.${key}`;
    const hasOwn = existing !== undefined && Object.prototype.hasOwnProperty.call(existing, key);
    const oldValue = hasOwn ? existing[key] : undefined;
    if (genValue && typeof genValue === "object") {
      merged[key] = mergeMissing(
        oldValue && typeof oldValue === "object" ? oldValue : undefined,
        genValue,
        here,
        stats,
      );
    } else if (hasOwn) {
      merged[key] = oldValue;
      stats.kept++;
      if (oldValue !== genValue) stats.differing++;
    } else {
      merged[key] = genValue;
      stats.added++;
    }
  }
  return merged;
}

/** 统计 existing 里存在、但 generated 已删掉的键数量。 */
function countRemoved(existing, generated) {
  let n = 0;
  for (const [key, value] of Object.entries(existing)) {
    const gen = generated[key];
    if (gen === undefined) {
      n += countStrings(value) || 1;
    } else if (value && typeof value === "object" && gen && typeof gen === "object") {
      n += countRemoved(value, gen);
    }
  }
  return n;
}

/** 是否覆盖 zh-HK 里的人工译文（默认不覆盖，只补缺失的词条）。 */
const FORCE = process.argv.includes("--force");

// 排序固定遍历顺序（`readdirSync` 的顺序依文件系统而定），保证输出可复现。
const namespaces = readdirSync(srcDir)
  .filter((f) => f.endsWith(".ts") && f !== "index.ts")
  .map((f) => f.replace(/\.ts$/, ""))
  .sort();

let changed = 0;

for (const ns of namespaces) {
  const file = `${ns}.ts`;
  const generated = convertValues(loadDefaultExport(join(srcDir, file)));
  const dest = join(outDir, file);
  const existing = readExistingLocale(dest);

  let final = generated;
  let header =
    "// 本文件由 scripts/generate-zh-hk.mjs 自动生成（源：zh-CN/" + file + "），请勿手改";
  let note = "";
  let mustWrite = !existing || FORCE;

  if (existing && !FORCE) {
    const stats = { added: 0, kept: 0, differing: 0 };
    final = mergeMissing(existing.value, generated, file, stats);
    if (existing.header) header = existing.header;
    const removed = countRemoved(existing.value, generated);
    note = `（新增 ${stats.added}／保留 ${stats.kept}，其中 ${stats.differing} 条与 OpenCC 结果不同，按人工译文保留`;
    note += removed > 0 ? `，zh-CN 已删除 ${removed} 条）` : "）";
    // 没有新增词条就整个文件都不动：zh-HK 里有手写的注释和人工排版，
    // 重写一遍只会把它们（和 diff）搅乱。
    mustWrite = stats.added > 0;
  } else if (existing && FORCE) {
    note = "（--force：已用 OpenCC 结果覆盖人工译文）";
  }

  totalKeys += countStrings(final);
  if (!mustWrite) {
    console.log(`⏭️ ${file}: 无新增词条${note}`);
    continue;
  }

  changed++;
  // 走仓库的 prettier 配置（键不加引号、尾逗号、printWidth…）：prettier 的 API
  // 不会自己读配置文件（CLI 才会），要先 resolveConfig；否则输出和仓库风格不一致，
  // 提交时又被 lint-staged 改一遍，diff 里全是噪音。
  const source = `${header}\nexport default ${JSON.stringify(final, null, 2)}\n`;
  const options = (await prettier.resolveConfig(dest)) ?? {};
  writeFileSync(dest, await prettier.format(source, { ...options, filepath: dest }), "utf-8");
  console.log(`✅ ${file}: ${countStrings(final)} 条 ${note}`.trimEnd());
}

// 聚合 index.ts（结构对齐 zh-CN/index.ts，命名空间固定）。
// 已经列全了就不动它：现有文件的手写说明（哪些模块是人工维护的）要留着。
const indexPath = join(outDir, "index.ts");
let existingIndex = "";
try {
  existingIndex = readFileSync(indexPath, "utf-8");
} catch {
  existingIndex = "";
}
const missingNamespaces = namespaces.filter(
  (ns) => !new RegExp(`^\\s*${ns},?\\s*$`, "m").test(existingIndex),
);
if (!existingIndex || missingNamespaces.length > 0) {
  const header = existingIndex.startsWith("//")
    ? existingIndex
        .split("\n")
        .filter((l) => l.startsWith("//"))
        .join("\n")
    : "// 本文件由 scripts/generate-zh-hk.mjs 自动生成，请勿手改";
  const imports = namespaces.map((ns) => `import ${ns} from "./${ns}";`).join("\n");
  const body = namespaces.map((ns) => `  ${ns},`).join("\n");
  const source = `${header}\n${imports}\n\nexport default {\n${body}\n};\n`;
  const options = (await prettier.resolveConfig(indexPath)) ?? {};
  writeFileSync(
    indexPath,
    await prettier.format(source, { ...options, filepath: indexPath }),
    "utf-8",
  );
  console.log(
    `✅ index.ts: 已重新生成${missingNamespaces.length ? `（补 ${missingNamespaces.join(", ")}）` : ""}`,
  );
} else {
  console.log("⏭️ index.ts: 命名空间已齐全，未改动");
}

console.log(
  `📄 共 ${totalKeys} 条词条，${changed} 个文件被写入${FORCE ? "（--force 覆盖模式）" : ""}`,
);

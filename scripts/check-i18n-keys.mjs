#!/usr/bin/env node
/**
 * i18n 静态 key 一致性检查。
 *
 * 检查范围：
 * 1. src 源码中静态写死的 $t('settings.*') / t('settings.*') / $t('pet.*') / t('pet.*')；
 * 2. src/locales/schema-i18n.ts 的映射表（scriptEditor.schema.* 引用，
 *    以及 emotionLabelOf 动态拼出的 settings.characterCreate.emotions.*）；
 * 3. 上述 key 在 zh-CN 基线词条中必须存在；其余三种语言若提供了对应词条，
 *    其层级结构（叶子值/嵌套对象）必须与 zh-CN 基线一致，缺翻译则按项目约定回落中文。
 *
 * 说明：本脚本不要求四语文件整体结构完全一致（历史上各语言存在增量补齐差异，
 * 运行时靠 fallbackLocale 兜底），但代码直接引用的 key 在 zh-CN 中缺失时，
 * 所有语言都会显示原始 key，必须拦截；非基线语言若存在同 key 但结构错误，
 * 说明是错误翻译结构，也必须拦截。
 *
 * 退出码：0 = PASS；1 = 存在缺失/结构不一致。
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = path.join(root, "src");
const localesDir = path.join(root, "src", "locales");
const localeNames = ["en", "zh-CN", "zh-HK", "ja"];

/** 递归收集目录下指定扩展名文件 */
function walkFiles(dir, exts) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = path.join(dir, entry);
    if (full === localesDir) continue; // 词条定义文件本身不参与引用扫描
    const stat = statSync(full);
    if (stat.isDirectory()) out.push(...walkFiles(full, exts));
    else if (exts.includes(path.extname(full))) out.push(full);
  }
  return out;
}

/** 去掉 JS/TS 注释（保留字符串字面量内容） */
function stripComments(text) {
  let out = "";
  let i = 0;
  let quote = null;
  while (i < text.length) {
    const ch = text[i];
    const next = text[i + 1];
    if (quote) {
      out += ch;
      if (ch === "\\") {
        out += next ?? "";
        i += 2;
        continue;
      }
      if (ch === quote) quote = null;
      i += 1;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === "`") {
      quote = ch;
      out += ch;
      i += 1;
      continue;
    }
    if (ch === "/" && next === "/") {
      while (i < text.length && text[i] !== "\n") i += 1;
      out += "\n";
      continue;
    }
    if (ch === "/" && next === "*") {
      i += 2;
      while (i < text.length && !(text[i] === "*" && text[i + 1] === "/")) i += 1;
      i += 2;
      out += " ";
      continue;
    }
    out += ch;
    i += 1;
  }
  return out;
}

/** 扫描 src 中的静态 settings./pet. 词条引用，返回 Map<key, locations> */
function scanSourceRefs() {
  const refs = new Map();
  const add = (key, file, line) => {
    if (!refs.has(key)) refs.set(key, []);
    refs.get(key).push(`${path.relative(root, file)}:${line}`);
  };
  // 只匹配单/双引号静态字符串；模板字符串插值跳过（属于动态 key，无法静态核对）
  const keyPattern = /(?:\$t|\bt)\s*\(\s*(['"])((?:settings|pet)\.[A-Za-z0-9_.{}]*)\1\s*[,)]/g;
  for (const file of walkFiles(srcDir, [".ts", ".tsx", ".js", ".mjs", ".vue"])) {
    const text = stripComments(readFileSync(file, "utf8"));
    for (const match of text.matchAll(keyPattern)) {
      const key = match[2];
      const line = text.slice(0, match.index).split("\n").length;
      add(key, file, line);
    }
  }
  return refs;
}

/** 解析 schema-i18n.ts 中的映射表引用 */
function scanSchemaI18nRefs() {
  const file = path.join(localesDir, "schema-i18n.ts");
  const text = stripComments(readFileSync(file, "utf8"));
  const refs = new Map();
  const add = (key, line) => {
    if (!refs.has(key)) refs.set(key, []);
    refs.get(key).push(`src/locales/schema-i18n.ts:${line}`);
  };

  // text(...) 的入参都会自动加上 scriptEditor.schema. 前缀：
  // 直接收集所有形如 section.key 的映射值，再统一补前缀校验。
  const schemaValuePattern =
    /["'](event|category|field|hint|placeholder|option|unlock|particle)\.[A-Za-z0-9.]+["']/g;
  for (const match of text.matchAll(schemaValuePattern)) {
    add(
      `scriptEditor.schema.${match[0].slice(1, -1)}`,
      text.slice(0, match.index).split("\n").length
    );
  }

  // EMOTION_SLUGS 的 value 是 settings.characterCreate.emotions.* 的尾部路径
  const emotionBlock = text.match(/const\s+EMOTION_SLUGS[^=]*=\s*\{([\s\S]*?)\n\};/);
  if (emotionBlock) {
    const entryPattern = /^\s*"[^"]+"\s*:\s*"([A-Za-z0-9]+)",?\s*$/gm;
    const blockStartLine = text.slice(0, emotionBlock.index).split("\n").length;
    for (const match of emotionBlock[1].matchAll(entryPattern)) {
      const line = blockStartLine + emotionBlock[1].slice(0, match.index).split("\n").length;
      add(`settings.characterCreate.emotions.${match[1]}`, line);
    }
  }
  return refs;
}

/** 简单词条源加载：这些 locale 文件都是纯对象字面量 export default，剥离后直接求值 */
function loadLocaleModule(relPath) {
  const source = stripComments(readFileSync(path.join(root, relPath), "utf8"));
  const body = source.replace(/export\s+default\s*/, "return ");
  // eslint-disable-next-line no-new-func
  const value = Function(body)();
  if (!value || typeof value !== "object") {
    throw new Error(`无法解析词条文件: ${relPath}`);
  }
  return value;
}

/** 按点分路径读取嵌套值；路径中断返回 undefined */
function getByPath(obj, keyPath) {
  let cur = obj;
  for (const part of keyPath.split(".")) {
    if (cur == null || typeof cur !== "object" || !(part in cur)) return undefined;
    cur = cur[part];
  }
  return cur;
}

/** 叶子值/对象节点类型：用于四语结构一致性比较 */
function nodeKind(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? "object" : "leaf";
}

/** 校验引用集合，返回错误列表 */
function checkRefs(refs, localeTrees) {
  const errors = [];
  let fallbackCount = 0;
  const sorted = [...refs.entries()].sort(([a], [b]) => a.localeCompare(b));
  for (const [key, locations] of sorted) {
    const baseline = getByPath(localeTrees["zh-CN"], key);
    if (baseline === undefined) {
      errors.push({
        type: "missing-baseline",
        key,
        locales: localeNames,
        locations: [...new Set(locations)].sort(),
      });
      continue;
    }
    for (const locale of ["en", "zh-HK", "ja"]) {
      const value = getByPath(localeTrees[locale], key);
      if (value === undefined) {
        fallbackCount += 1; // 项目约定：其他语言可缺词条，运行时回落 zh-CN
        continue;
      }
      if (nodeKind(value) !== nodeKind(baseline)) {
        errors.push({
          type: "structure-mismatch",
          key,
          locale,
          baselineKind: nodeKind(baseline),
          localeKind: nodeKind(value),
          locations: [...new Set(locations)].sort(),
        });
      }
    }
  }
  return { errors, fallbackCount };
}

/** 主流程 */
function main() {
  const sourceRefs = scanSourceRefs();
  const schemaRefs = scanSchemaI18nRefs();
  const refs = new Map();
  for (const [key, locations] of [...sourceRefs, ...schemaRefs]) {
    if (!refs.has(key)) refs.set(key, []);
    const seen = new Set(refs.get(key));
    for (const loc of locations) {
      if (!seen.has(loc)) {
        seen.add(loc);
        refs.get(key).push(loc);
      }
    }
  }

  const localeTrees = {};
  for (const locale of localeNames) {
    localeTrees[locale] = {
      settings: loadLocaleModule(`src/locales/${locale}/settings.ts`),
      pet: loadLocaleModule(`src/locales/${locale}/pet.ts`),
      scriptEditor: loadLocaleModule(`src/locales/${locale}/scriptEditor.ts`),
    };
  }

  const { errors, fallbackCount } = checkRefs(refs, localeTrees);

  console.log(
    `i18n key check: 源码引用 ${sourceRefs.size} 个，schema-i18n 引用 ${schemaRefs.size} 个，去重后 ${refs.size} 个`
  );
  if (fallbackCount > 0) {
    console.log(`NOTE: ${fallbackCount} 个非 zh-CN 引用词条缺失，按项目约定运行时回落中文`);
  }
  if (errors.length === 0) {
    console.log("PASS: 静态 i18n 引用在 zh-CN 中齐备，且四语已提供词条的结构一致");
    return 0;
  }

  console.log(`FAIL: 发现 ${errors.length} 处词条问题`);
  for (const error of errors) {
    if (error.type === "missing-baseline") {
      console.log(`  [zh-CN 缺失] ${error.key}`);
    } else {
      console.log(
        `  [结构不一致] ${error.locale}: ${error.key} (zh-CN 为 ${error.baselineKind}，当前为 ${error.localeKind})`
      );
    }
    for (const loc of error.locations) console.log(`    引用位置: ${loc}`);
  }
  return 1;
}

process.exit(main());

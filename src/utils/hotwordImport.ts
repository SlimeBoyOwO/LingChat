import { hotwordLimitKind } from "../api/services/hotword";
import { HOTWORD_DEFAULT_WEIGHT, HOTWORD_WEIGHT_OPTIONS } from "./hotwordRows";

/**
 * 批量热词导入的纯函数解析内核。
 *
 * 只负责把用户粘贴的文本拆成可导入行与异常明细，不依赖 Vue，也不调用 IPC。
 * 三个作用域（角色 / 全局 / 词库）共用这一份规则，避免各自的校验口径漂移。
 */

export interface ImportRow {
  text: string;
  weight: number;
}

export interface ImportDetail {
  kind: "invalid" | "overCapacity";
  line: number;
  label?: string;
  text: string;
  weight: number | string;
  reason: ImportReason;
}

export type ImportReason =
  | "empty"
  | "tooLongChars"
  | "tooLongSegments"
  | "badWeight"
  | "unclosedWeight"
  | "overCapacity";

export interface ImportReview {
  rows: ImportRow[];
  importableCount: number;
  newCount: number;
  /** 与当前草稿已有词条重复、将通过去重合并的行数 */
  mergeCount: number;
  /** 粘贴内容内部重复、合并到首现行的行数（不计入可导入） */
  inputDuplicateCount: number;
  invalidCount: number;
  overCapacityCount: number;
  ignoredCount: number;
  details: ImportDetail[];
}

interface ParsedRow extends ImportRow {
  line: number;
  label?: string;
}

interface TrailingBracket {
  content: string;
  index: number;
}

type WeightParse =
  | { kind: "none" }
  | { kind: "ok"; text: string; weight: number; rawWeight: string }
  | { kind: "bad"; text: string; rawWeight: string }
  | { kind: "unclosed"; text: string };

const VALID_WEIGHTS = new Set<number>(HOTWORD_WEIGHT_OPTIONS);

const normalizeDigits = (text: string) =>
  text.replace(/[０-９]/g, (ch) => String(ch.charCodeAt(0) - 0xff10));

/** 返回合法权重；空、非数字、非法档位一律 null。 */
function parseWeightToken(raw: string): number | null {
  const text = normalizeDigits(raw.trim());
  if (!/^\d+$/.test(text)) return null;
  const weight = Number(text);
  return VALID_WEIGHTS.has(weight) ? weight : null;
}

/** 剥掉包住整条文本的成对 Markdown 包装，最多两轮。 */
function stripOuterWrappers(input: string): string {
  let text = input.trim();
  for (let round = 0; round < 2; round += 1) {
    let changed = false;
    for (const wrapper of ["**", "__", "`"]) {
      if (text.length > wrapper.length * 2 && text.startsWith(wrapper) && text.endsWith(wrapper)) {
        text = text.slice(wrapper.length, text.length - wrapper.length).trim();
        changed = true;
        break;
      }
    }
    if (!changed) break;
  }
  return text;
}

/** 剥掉行首引用标记；引用块不是忽略，剥完继续解析。 */
function stripBlockquote(line: string): string {
  let text = line;
  while (text.startsWith(">")) {
    text = text.slice(1).trimStart();
  }
  return text.trim();
}

/** 行首序号 / 项目符号只剥一次。 */
function stripListMarker(line: string): { text: string; label?: string } {
  // 项目符号后必须有空白，避免吃掉 -40℃ 与 *NSYNC
  const bullet = line.match(/^[-*+•]\s+/);
  if (bullet) return { text: line.slice(bullet[0].length).trim() };

  const chinese = line.match(/^([一二三四五六七八九十百]+)、\s*/);
  if (chinese) {
    return { text: line.slice(chinese[0].length).trim(), label: chinese[1] };
  }

  // `.` / `．` 后紧跟数字时不是序号，保留 3.5mm / 2026.10.20
  const numeric = line.match(/^([0-9０-９]+)(?:[.．](?![0-9０-９])|[、)])\s*/);
  if (numeric) {
    // 全角序号只作为标签，归一成 ASCII 数字便于异常明细展示
    return { text: line.slice(numeric[0].length).trim(), label: normalizeDigits(numeric[1]) };
  }

  return { text: line };
}

/** 行尾最后一个括号组；括号内不允许嵌套。 */
function findTrailingBracket(text: string): TrailingBracket | null {
  const match = text.match(/([（(])([^（）()]*)([）)])\s*$/);
  if (!match || match.index === undefined) return null;
  return { content: match[2], index: match.index };
}

/**
 * 提取行尾权重。
 *
 * 两种保留语法：`（权重 5）` 与纯数字 `（5）`。括号内是其它文字时不做处理，
 * 继续留在词条正文里（如 `诺一（限定）`）。
 */
function parseTrailingWeight(text: string): WeightParse {
  if (/[（(]\s*权重\s*[^（）()]*$/.test(text)) {
    return { kind: "unclosed", text };
  }

  const bracket = findTrailingBracket(text);
  if (!bracket) return { kind: "none" };

  const content = bracket.content.trim();
  const before = text.slice(0, bracket.index).trim();

  if (content.startsWith("权重")) {
    const rest = content.slice(2).trim();
    const token = rest.replace(/^[：:＝=]/, "").trim();
    const weight = parseWeightToken(token);
    if (weight === null) return { kind: "bad", text: before, rawWeight: content };
    return { kind: "ok", text: before, weight, rawWeight: content };
  }

  if (/^[0-9０-９]+$/.test(content)) {
    const weight = parseWeightToken(content);
    if (weight === null) return { kind: "bad", text: before, rawWeight: content };
    return { kind: "ok", text: before, weight, rawWeight: content };
  }

  return { kind: "none" };
}

/** Markdown 中与词表无关的整行噪音。 */
function isNoiseLine(line: string): boolean {
  return (
    /^```/.test(line) ||
    /^~~~/.test(line) ||
    /^#{1,6}\s/.test(line) ||
    /^(?:-{3,}|\*{3,}|_{3,})$/.test(line)
  );
}

/**
 * 解析并审查粘贴文本。
 *
 * `existingRows` 是当前草稿（不是数据库）；容量基数按它的原始行数计算。
 * 返回的 `rows` 只包含可提交的行（额度内新增 + 合并行）。
 */
export function parseHotwordImport(
  text: string,
  defaultWeight: number,
  existingRows: { text: string }[],
  capacity: number,
): ImportReview {
  const fallbackWeight = parseWeightToken(String(defaultWeight)) ?? HOTWORD_DEFAULT_WEIGHT;
  const normalized = text.replace(/^﻿/, "").replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");
  const existingKeys = new Set(existingRows.map((row) => row.text.trim()).filter(Boolean));

  const parsed: ParsedRow[] = [];
  const details: ImportDetail[] = [];
  let ignoredCount = 0;
  let invalidCount = 0;

  lines.forEach((rawLine, index) => {
    const lineNo = index + 1;
    const raw = rawLine.trim();
    if (!raw) return;

    // 先 trim 再剥引用标记，缩进的全角/半角空格不会挡住行首 >
    const quoted = stripBlockquote(raw);
    if (!quoted) return;
    if (isNoiseLine(quoted)) {
      ignoredCount += 1;
      return;
    }

    const marker = stripListMarker(quoted);
    if (!marker.text) {
      invalidCount += 1;
      details.push({
        kind: "invalid",
        line: lineNo,
        label: marker.label,
        text: raw,
        weight: fallbackWeight,
        reason: "empty",
      });
      return;
    }

    let content = stripOuterWrappers(marker.text);
    const weightResult = parseTrailingWeight(content);
    let weight = fallbackWeight;

    if (weightResult.kind === "unclosed") {
      invalidCount += 1;
      details.push({
        kind: "invalid",
        line: lineNo,
        label: marker.label,
        text: raw,
        weight: "",
        reason: "unclosedWeight",
      });
      return;
    }

    if (weightResult.kind === "bad") {
      invalidCount += 1;
      details.push({
        kind: "invalid",
        line: lineNo,
        label: marker.label,
        text: raw,
        weight: weightResult.rawWeight,
        reason: "badWeight",
      });
      return;
    }

    if (weightResult.kind === "ok") {
      content = weightResult.text;
      weight = weightResult.weight;
    }

    content = stripOuterWrappers(content);
    if (!content) {
      invalidCount += 1;
      details.push({
        kind: "invalid",
        line: lineNo,
        label: marker.label,
        text: raw,
        weight: weight,
        reason: "empty",
      });
      return;
    }

    const limit = hotwordLimitKind(content);
    if (limit !== "ok") {
      invalidCount += 1;
      details.push({
        kind: "invalid",
        line: lineNo,
        label: marker.label,
        text: content,
        weight: weight,
        reason: limit === "segments" ? "tooLongSegments" : "tooLongChars",
      });
      return;
    }

    parsed.push({
      line: lineNo,
      label: marker.label,
      text: content,
      weight,
    });
  });

  // 粘贴内容内部去重：保留首现行，权重取最大值
  const firstByText = new Map<string, ParsedRow>();
  const uniqueRows: ParsedRow[] = [];
  let inputDuplicateCount = 0;
  for (const row of parsed) {
    const key = row.text.trim();
    const first = firstByText.get(key);
    if (first) {
      first.weight = Math.max(first.weight, row.weight);
      inputDuplicateCount += 1;
    } else {
      firstByText.set(key, row);
      uniqueRows.push(row);
    }
  }

  // 与当前草稿比较，并按顺序消耗剩余容量
  let remaining = Math.max(0, capacity - existingRows.length);
  let newCount = 0;
  let mergeCount = 0;
  let overCapacityCount = 0;
  const rows: ImportRow[] = [];

  for (const row of uniqueRows) {
    if (existingKeys.has(row.text)) {
      mergeCount += 1;
      rows.push({ text: row.text, weight: row.weight });
      continue;
    }
    if (remaining > 0) {
      remaining -= 1;
      newCount += 1;
      rows.push({ text: row.text, weight: row.weight });
      continue;
    }
    overCapacityCount += 1;
    details.push({
      kind: "overCapacity",
      line: row.line,
      label: row.label,
      text: row.text,
      weight: row.weight,
      reason: "overCapacity",
    });
  }

  return {
    rows,
    importableCount: rows.length,
    newCount,
    mergeCount,
    inputDuplicateCount,
    invalidCount,
    overCapacityCount,
    ignoredCount,
    details,
  };
}

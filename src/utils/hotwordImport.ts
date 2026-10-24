import { hotwordLimitKind } from "../api/services/hotword";
import { HOTWORD_WEIGHT_OPTIONS } from "./hotwordRows";

/**
 * 批量热词导入的纯函数解析内核。
 *
 * 只接受一种行格式：序号.词条(权重)，例如 `1.钦灵(5)`。
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
  "badFormat" | "tooLongChars" | "tooLongSegments" | "badWeight" | "overCapacity";

export interface ImportReview {
  rows: ImportRow[];
  importableCount: number;
  newCount: number;
  /** 与当前草稿目标分类内已有词条重复、将通过去重合并的行数 */
  mergeCount: number;
  /** 粘贴内容内部重复、合并到首现行的行数（不计入可导入） */
  inputDuplicateCount: number;
  invalidCount: number;
  overCapacityCount: number;
  details: ImportDetail[];
}

interface ParsedRow extends ImportRow {
  line: number;
  label?: string;
}

const VALID_WEIGHTS = new Set<number>(HOTWORD_WEIGHT_OPTIONS);

/** 唯一的行格式：序号.词条(权重)；全角数字、句点与括号视为同一写法。 */
const LINE_PATTERN = /^([0-9０-９]+)[.．]\s*(.+?)\s*[（(]\s*([0-9０-９]+)\s*[）)]$/;

const normalizeDigits = (text: string) =>
  text.replace(/[０-９]/g, (ch) => String(ch.charCodeAt(0) - 0xff10));

function parseWeightToken(raw: string): number | null {
  const text = normalizeDigits(raw.trim());
  if (!/^\d+$/.test(text)) return null;
  const weight = Number(text);
  return VALID_WEIGHTS.has(weight) ? weight : null;
}

/**
 * 解析并审查粘贴文本。
 *
 * `existingRows` 是当前草稿（不是数据库）；容量基数按它的原始行数计算。
 * `targetCategory` 仅词库传入：判重只看目标分类内的已有词条，同词可在别的分类里。
 * 返回的 `rows` 只包含可提交的行（额度内新增 + 合并行）。
 */
export function parseHotwordImport(
  text: string,
  existingRows: { text: string; category_index?: number | null }[],
  capacity: number,
  targetCategory?: number | null,
): ImportReview {
  const normalized = text.replace(/^﻿/, "").replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");
  const existingKeys = new Set(
    existingRows
      .filter(
        (row) => targetCategory === undefined || (row.category_index ?? null) === targetCategory,
      )
      .map((row) => row.text.trim())
      .filter(Boolean),
  );

  const parsed: ParsedRow[] = [];
  const details: ImportDetail[] = [];
  let invalidCount = 0;

  lines.forEach((rawLine, index) => {
    const lineNo = index + 1;
    const line = rawLine.trim();
    if (!line) return;

    const match = line.match(LINE_PATTERN);
    if (!match) {
      invalidCount += 1;
      details.push({
        kind: "invalid",
        line: lineNo,
        text: line,
        weight: "",
        reason: "badFormat",
      });
      return;
    }

    const label = normalizeDigits(match[1]);
    const content = match[2].trim();

    // 空白正文也能匹配行格式（如 `1. (5)`）；必须在进入容量计算前拦掉，
    // 否则会生成一条后端保存时被丢弃的空词条，还白白占掉一个新增额度。
    if (content === "") {
      invalidCount += 1;
      details.push({
        kind: "invalid",
        line: lineNo,
        label,
        text: line,
        weight: "",
        reason: "badFormat",
      });
      return;
    }

    const rawWeight = normalizeDigits(match[3]);
    const weight = parseWeightToken(rawWeight);

    if (weight === null) {
      invalidCount += 1;
      details.push({
        kind: "invalid",
        line: lineNo,
        label,
        text: content,
        weight: rawWeight,
        reason: "badWeight",
      });
      return;
    }

    const limit = hotwordLimitKind(content);
    if (limit !== "ok") {
      invalidCount += 1;
      details.push({
        kind: "invalid",
        line: lineNo,
        label,
        text: content,
        weight,
        reason: limit === "segments" ? "tooLongSegments" : "tooLongChars",
      });
      return;
    }

    parsed.push({
      line: lineNo,
      label,
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
    details,
  };
}

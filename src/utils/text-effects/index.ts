/**
 * 文字演出标签解析器（issue #658 对话文字动画）。
 *
 * 语法（HTML/XML 风格嵌入式标签）：
 *   <emphasis>文字</emphasis>          加重强调（加粗 + 放大呼吸）
 *   <shake>文字</shake>                抖动
 *   <blur>文字</blur>                  模糊不清
 *   <float>文字</float>                飘动
 *   强度参数 level（可选）：low / medium（默认）/ high
 *   示例：<shake level="high">很好看！</shake>
 *
 * 兜底规则（防止 LLM 写错或正文符号误伤）：
 *   - 仅识别已知标签名（含中文别名），未知/未闭合标签一律按普通文本
 *   - 内容长度 1~30 字符（UTF-16 code unit，与 TypeWriter 逐字下标一致）
 *   - 内容不能跨行（不含 \n）
 *   - 支持嵌套，同一字符取最内层标签的效果
 */

export type TextEffect = "none" | "emphasis" | "shake" | "blur" | "float";

export type TextEffectLevel = "low" | "medium" | "high";

export interface TextEffectSegment {
  /** 起始下标（含，UTF-16 code unit，与 text.charAt 一致） */
  start: number;
  /** 结束下标（不含） */
  end: number;
  effect: TextEffect;
  level: TextEffectLevel;
}

/** stripTextEffects 的返回：去标签后的显示文本 + 对齐显示文本下标的动画片段 */
export interface StrippedText {
  text: string;
  segments: TextEffectSegment[];
}

/** 标签名 → 动画效果（英文 + 中文别名归一化） */
const TAG_EFFECT_MAP: Record<string, TextEffect> = {
  emphasis: "emphasis",
  加重: "emphasis",
  shake: "shake",
  抖动: "shake",
  blur: "blur",
  模糊: "blur",
  float: "float",
  飘动: "float",
};

const MAX_CONTENT_LEN = 30;
const MIN_CONTENT_LEN = 1;

interface ParsedTag {
  isClose: boolean;
  effect: TextEffect;
  level: TextEffectLevel;
  raw: string;
}

/** 在 index 处尝试匹配一个已知演出标签（<name attrs> 或 </name>），失败返回 null */
function matchTagAt(text: string, index: number): ParsedTag | null {
  if (text.charAt(index) !== "<") return null;
  const m = /^<(\/)?\s*([A-Za-z\u4e00-\u9fff]+)([^>]*)>/.exec(text.slice(index));
  if (!m) return null;
  const effect = TAG_EFFECT_MAP[m[2].toLowerCase()];
  if (!effect) return null;
  const attrs: Record<string, string> = {};
  const attrRe = /([A-Za-z\u4e00-\u9fff-]+)\s*=\s*"([^"]*)"/g;
  let am: RegExpExecArray | null;
  while ((am = attrRe.exec(m[3]))) {
    attrs[am[1].toLowerCase()] = am[2];
  }
  return {
    isClose: m[1] !== undefined,
    effect,
    level: parseLevel(attrs["level"]),
    raw: m[0],
  };
}

/** level 属性解析：low/high（忽略大小写）、1/3 → 对应档；其余（含缺省）→ medium */
function parseLevel(raw: string | undefined): TextEffectLevel {
  if (!raw) return "medium";
  const v = raw.trim().toLowerCase();
  if (v === "low" || v === "1") return "low";
  if (v === "high" || v === "3") return "high";
  return "medium";
}

/** 内容合法性：长度 1~30、不跨行 */
function isValidContent(content: string): boolean {
  if (content.length < MIN_CONTENT_LEN || content.length > MAX_CONTENT_LEN) return false;
  if (content.includes("\n")) return false;
  return true;
}

/** 统计 [0, pos) 区间内被移除的字符数（remove 区间按起点排序、互不重叠） */
function removedCountUpTo(remove: [number, number][], pos: number): number {
  let n = 0;
  for (const [a, b] of remove) {
    if (b <= pos) n += b - a;
    else if (a < pos) n += pos - a;
    else break;
  }
  return n;
}

/**
 * 去掉演出标签，得到显示文本与对齐显示文本下标的动画片段（含 level）。
 * 渲染层（逐字显示 / 历史展示）应使用此函数：标签文本不显示，仅内容渲染动画。
 */
export function stripTextEffects(raw: string): StrippedText {
  // ── 第一遍：识别合法标签对（严格嵌套），记录移除区间与内容区间 ──
  const remove: [number, number][] = [];
  const pairs: {
    contentStart: number;
    contentEnd: number;
    effect: TextEffect;
    level: TextEffectLevel;
  }[] = [];
  const stack: {
    effect: TextEffect;
    level: TextEffectLevel;
    openStart: number;
    openEnd: number;
  }[] = [];

  let i = 0;
  while (i < raw.length) {
    const tag = matchTagAt(raw, i);
    if (!tag) {
      i++;
      continue;
    }
    if (!tag.isClose) {
      stack.push({
        effect: tag.effect,
        level: tag.level,
        openStart: i,
        openEnd: i + tag.raw.length,
      });
    } else {
      const top = stack[stack.length - 1];
      if (top && top.effect === tag.effect) {
        const open = stack.pop()!;
        const contentStart = open.openEnd;
        const contentEnd = i;
        const content = raw.slice(contentStart, contentEnd);
        if (isValidContent(content)) {
          remove.push([open.openStart, open.openEnd]);
          remove.push([i, i + tag.raw.length]);
          pairs.push({ contentStart, contentEnd, effect: open.effect, level: open.level });
        }
        // 内容非法：整对按普通文本（开/闭标签都保留）
      }
      // 交叉/无匹配闭合：按普通文本保留
    }
    i += tag.raw.length;
  }
  remove.sort((a, b) => a[0] - b[0]);

  // ── 第二遍：重建输出文本 + 计算对齐片段的输出区间 ──
  let out = "";
  let j = 0;
  let ri = 0;
  while (j < raw.length) {
    if (ri < remove.length && j === remove[ri][0]) {
      j = remove[ri][1];
      ri++;
      continue;
    }
    out += raw.charAt(j);
    j++;
  }

  const segments: TextEffectSegment[] = pairs.map((p) => {
    const segStart = p.contentStart - removedCountUpTo(remove, p.contentStart);
    const removedInside =
      removedCountUpTo(remove, p.contentEnd) - removedCountUpTo(remove, p.contentStart);
    const contentOutLen = p.contentEnd - p.contentStart - removedInside;
    return { start: segStart, end: segStart + contentOutLen, effect: p.effect, level: p.level };
  });

  return { text: out, segments };
}

/** 查询某个字符下标所属的最内层动画片段（嵌套时取范围最小者），无则返回 null */
export function textEffectSegmentAt(
  index: number,
  segments: TextEffectSegment[]
): TextEffectSegment | null {
  let best: TextEffectSegment | null = null;
  for (const seg of segments) {
    if (index >= seg.start && index < seg.end) {
      if (!best || seg.end - seg.start < best.end - best.start) best = seg;
    }
  }
  return best;
}

/** 查询某个字符下标（UTF-16 code unit）所属的动画效果 */
export function textEffectAt(index: number, segments: TextEffectSegment[]): TextEffect {
  return textEffectSegmentAt(index, segments)?.effect ?? "none";
}

/** 动画效果对应的 CSS class（外层包裹 span 使用；'none' 返回空串） */
export function textEffectClass(effect: TextEffect): string {
  switch (effect) {
    case "emphasis":
      return "tw-fx-emphasis";
    case "shake":
      return "tw-fx-shake";
    case "blur":
      return "tw-fx-blur";
    case "float":
      return "tw-fx-float";
    default:
      return "";
  }
}

/**
 * level 强度对应的 CSS 变量（内联到包裹 span 的 style；medium 走 CSS 默认值，返回空串）。
 * 各动画 keyframes 用 var(--fx-*) 消费这些变量。
 */
export function textEffectLevelStyle(effect: TextEffect, level: TextEffectLevel): string {
  if (level === "medium") return "";
  switch (effect) {
    case "shake": {
      const amp = level === "low" ? "1px" : "3px";
      const dur = level === "low" ? ".6s" : ".3s";
      return `--fx-amp:${amp};--fx-dur:${dur}`;
    }
    case "float": {
      const off = level === "low" ? "-2px" : "-7px";
      const dur = level === "low" ? "3.2s" : "1.6s";
      return `--fx-off:${off};--fx-dur:${dur}`;
    }
    case "blur": {
      const b = level === "low" ? "1.2px" : "3.2px";
      return `--fx-blur:${b}`;
    }
    case "emphasis": {
      const sc = level === "low" ? "1.05" : "1.22";
      const dur = level === "low" ? "2s" : "1.2s";
      return `--fx-scale:${sc};--fx-dur:${dur}`;
    }
    default:
      return "";
  }
}

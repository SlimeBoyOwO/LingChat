/**
 * 剧本「变量表达式 / 触发条件」的结构化解析与序列化。
 *
 * 编辑器底层写进 YAML 的仍是引擎认的字符串格式（`condition: "route == shop"`、
 * `content: "flag = warm"`），本文件负责在「表单结构化字段」和「字符串」之间互转，
 * 让作者不用手写类代码语法。纯函数、无 Vue 依赖，便于复用与自测。
 *
 * 解析逻辑刻意**镜像引擎**（`evaluate_condition` / `parse_variable_action`）：
 * - 条件：先拆 `!=`、再拆 `==`，否则视为裸变量真值判断；
 * - 赋值：正则拆 `变量 运算符 值`，值保持**原文**（不调引擎的 parse_value，
 *   否则 `random(1,6)` 会被消费成一个随机数、无法原样回写）。
 *
 * 无法解析的内容（如 `hp > 5`、旧版 name/value/op 形状）返回 null，
 * 由调用方只读展示 + 校验器兜底。
 */

// ============================================================
// 条件（condition） ⇄ 结构化
// ============================================================

export type ConditionRel = 'truthy' | 'eq' | 'neq'

export interface ConditionParts {
  /** 变量名 */
  var: string
  /** truthy = 裸变量真值判断（不带值） */
  rel: ConditionRel
  /** 仅 eq / neq 时有意义；展示时去掉引擎会剥掉的首尾引号 */
  value: string
}

/** 是否属于引擎支持的语法。可解析返回结构，否则返回 null（交给只读展示 + 校验器）。 */
export function parseCondition(raw: string | null | undefined): ConditionParts | null {
  const s = (raw ?? '').trim()
  if (!s) return null

  // 与引擎 evaluate_condition 一致：先找 !=，再找 ==，否则是裸变量
  const neq = s.indexOf('!=')
  const eq = s.indexOf('==')
  const op = neq !== -1 ? '!=' : eq !== -1 ? '==' : null
  const sep = op === '!=' ? neq : eq

  const varName = (op ? s.slice(0, sep) : s).trim()
  const value = op ? stripQuotes(s.slice(sep + 2).trim()) : ''

  if (!varName) return null
  // 引擎拿整段左侧当变量键查，带空格的变量名永远查不到 —— 不是合法写法
  if (/\s/.test(varName)) return null

  return { var: varName, rel: op === '!=' ? 'neq' : op === '==' ? 'eq' : 'truthy', value }
}

/** 序列化回引擎字符串。选了三段式但没填值，视为「未设置」（返回空串由上层删键）。 */
export function buildCondition(parts: ConditionParts): string {
  const v = parts.var.trim()
  if (!v) return ''
  if (parts.rel === 'truthy') return v
  const value = parts.value.trim()
  if (!value) return ''
  return parts.rel === 'eq' ? `${v} == ${value}` : `${v} != ${value}`
}

// ============================================================
// set_var 的 content（变量赋值表达式）⇄ 结构化
// ============================================================

export type VarOp = '=' | '+=' | '-='
export type VarValueKind = 'text' | 'number' | 'bool' | 'random'

export interface VarParts {
  var: string
  op: VarOp
  kind: VarValueKind
  /** text/number 的原文；bool 为 'true'/'false' */
  value: string
  /** kind === 'random' 时的区间 */
  randomMin?: number
  randomMax?: number
}

// 运算符「+=/-=」必须排在「=」前面，否则 `count += 1` 会先匹配成 `count +` 后接 `=`。
const VAR_RE = /^\s*(\S+)\s*(\+=|-=|=)\s*(.*?)\s*$/

function valueKind(raw: string): VarValueKind {
  const s = raw.trim()
  if (/^random\(\s*-?\d+\s*,\s*-?\d+\s*\)$/i.test(s)) return 'random'
  if (/^(true|false)$/i.test(s)) return 'bool'
  if (/^-?\d+(\.\d+)?$/.test(s)) return 'number'
  return 'text'
}

/** 解析赋值表达式。值保持原文，random 不消费随机数。 */
export function parseVarAction(content: string | null | undefined): VarParts | null {
  const s = (content ?? '').trim()
  if (!s) return null
  const m = VAR_RE.exec(s)
  if (!m) return null
  const [, name, opRaw, rawValue] = m
  const op = opRaw as VarOp
  if (!name) return null

  const kind = valueKind(rawValue)
  const parts: VarParts = { var: name, op, kind, value: rawValue.trim() }
  // 布尔值统一成小写，避免 TRUE/True 在布尔下拉里显示成「假」、一动就改变语义
  if (kind === 'bool') parts.value = parts.value.toLowerCase()
  if (kind === 'random') {
    const mm = /^random\(\s*(-?\d+)\s*,\s*(-?\d+)\s*\)$/i.exec(rawValue.trim())
    if (mm) {
      parts.randomMin = Number(mm[1])
      parts.randomMax = Number(mm[2])
    }
  }
  return parts
}

/** 序列化回引擎表达式。变量名或值未填 → 返回空串（由上层删键，视为未设置）。 */
export function buildVarAction(parts: VarParts): string {
  const v = parts.var.trim()
  if (!v) return ''
  const value = renderValue(parts)
  if (!value) return ''
  return `${v} ${parts.op} ${value}`
}

function renderValue(p: VarParts): string {
  switch (p.kind) {
    case 'text':
      return p.value.trim()
    case 'number': {
      const n = p.value.trim()
      return n
    }
    case 'bool':
      return p.value === 'true' ? 'true' : 'false'
    case 'random': {
      if (p.randomMin === undefined || p.randomMax === undefined) return ''
      if (!Number.isFinite(p.randomMin) || !Number.isFinite(p.randomMax)) return ''
      if (p.randomMax < p.randomMin) return ''
      return `random(${p.randomMin}, ${p.randomMax})`
    }
  }
}

/** 引擎 evaluate_condition / parse_value 会用 trim_matches 剥掉首尾引号，展示时同样处理。 */
function stripQuotes(s: string): string {
  const len = s.length
  if (len >= 2) {
    const first = s[0]
    const last = s[len - 1]
    if ((first === '"' && last === '"') || (first === "'" && last === "'")) {
      return s.slice(1, -1)
    }
  }
  return s
}

import type { ScriptEventData } from '@/api/services/script-editor'

/**
 * 把真实语料里高频复现的固定套路折叠成一行。
 *
 * 依据是官方六个剧本的实际写法：
 *
 * - **转场**：`角色退场 → 旁白* → 背景 → 特效? → 角色出场`。这套在
 *   想出去玩啦 的 main2/main3/main4/final 每章开头都出现一次，钦灵黄油
 *   的 Intro/intro 也是。
 * - **AI 互动轮次**：`AI对话 → 等待输入 → AI对话`。长章节里出现两三次，
 *   main4 里出现两次。
 *
 * 折叠后 main4 从 15 行降到 8 行，而且转场那行直接显示目标背景名，扫一眼
 * 就知道这段切到哪。这比按背景切段有用 —— 真实章节里通常只有一个背景，
 * 按背景切段等于没切。
 */

export interface FoldedSingle {
  kind: 'event'
  /** 稳定标识 */
  key: string
  /** 在原始 events 数组里的下标 */
  index: number
  event: ScriptEventData
}

export interface FoldedGroup {
  kind: 'group'
  /** 稳定标识。用行下标做 key 会在插入/删除事件后让展开态跟错行 */
  key: string
  /** 复合块类型，直接作为标签展示 */
  label: string
  /** 起始下标（含） */
  from: number
  /** 结束下标（不含） */
  to: number
  /** 一行摘要 */
  summary: string
  items: { index: number; event: ScriptEventData }[]
}

export type FoldedRow = FoldedSingle | FoldedGroup

const typeOf = (e: ScriptEventData | undefined): string =>
  typeof e?.type === 'string' ? (e.type as string) : ''

const str = (e: ScriptEventData | undefined, k: string): string => {
  const v = e?.[k]
  return typeof v === 'string' ? v : ''
}

const isHide = (e: ScriptEventData | undefined) =>
  typeOf(e) === 'modify_character' && str(e, 'action') === 'hide_character'

const isShow = (e: ScriptEventData | undefined) =>
  typeOf(e) === 'modify_character' && str(e, 'action') === 'show_character'

/**
 * 识别复合块。`enabled` 为 false 时原样返回逐条事件，
 * 让用户能一键退回「什么都不折叠」。
 */
export function foldEvents(events: ScriptEventData[], enabled = true): FoldedRow[] {
  if (!enabled) {
    return events.map((event, index) => ({ kind: 'event', key: `e${index}`, index, event }))
  }

  const rows: FoldedRow[] = []
  let i = 0

  const slice = (from: number, to: number) =>
    events.slice(from, to).map((event, k) => ({ index: from + k, event }))

  while (i < events.length) {
    // ---- 转场 ----
    if (isHide(events[i])) {
      let k = i + 1
      while (typeOf(events[k]) === 'narration') k++
      if (typeOf(events[k]) === 'background') {
        const bgIndex = k
        k++
        if (typeOf(events[k]) === 'background_effect') k++
        if (isShow(events[k])) k++
        rows.push({
          kind: 'group',
          key: `g-transition-${i}`,
          label: '转场',
          from: i,
          to: k,
          summary: str(events[bgIndex], 'imagePath') || '（未指定背景）',
          items: slice(i, k),
        })
        i = k
        continue
      }
    }

    // ---- AI 互动轮次 ----
    if (
      typeOf(events[i]) === 'ai_dialogue' &&
      typeOf(events[i + 1]) === 'input' &&
      typeOf(events[i + 2]) === 'ai_dialogue'
    ) {
      rows.push({
        kind: 'group',
        key: `g-ai-${i}`,
        label: 'AI 互动轮次',
        from: i,
        to: i + 3,
        summary: str(events[i], 'prompt') || '（无提示 · 纯靠上下文生成）',
        items: slice(i, i + 3),
      })
      i += 3
      continue
    }

    rows.push({ kind: 'event', key: `e${i}`, index: i, event: events[i] })
    i++
  }

  return rows
}

/** 第一个没有被折叠进复合块的事件下标，用来避免「选中项看不见」 */
export function firstVisibleIndex(events: ScriptEventData[], enabled = true): number {
  const first = foldEvents(events, enabled).find((r) => r.kind === 'event')
  return first && first.kind === 'event' ? first.index : 0
}

/** 给定事件下标，返回它所在复合块的下标（不在任何块里则返回 null） */
export function groupContaining(rows: FoldedRow[], eventIndex: number): number | null {
  for (let gi = 0; gi < rows.length; gi++) {
    const r = rows[gi]
    if (r.kind === 'group' && eventIndex >= r.from && eventIndex < r.to) return gi
  }
  return null
}

/** 一行摘要：把事件的关键字段浓缩成一句人话。
 * `mainRoleName` 为绑定羁绊人物的展示名（供 MAIN 显示），
 * `roleNameMap` 为 roleKey → aiName 映射（供剧本 NPC 显示名字而非键）。
 * 两者缺省时回退字面 MAIN / roleKey。 */
export function eventSummary(
  e: ScriptEventData,
  mainRoleName?: string,
  roleNameMap?: Map<string, string>,
): string {
  const t = typeOf(e)
  const s = (k: string) => str(e, k)
  const roleLabel = (k: string) => {
    const key = s(k)
    if (!key || key === 'MAIN') return mainRoleName || 'MAIN'
    return roleNameMap?.get(key) || key
  }

  switch (t) {
    case 'narration':
    case 'player':
      return s('text').replace(/\n/g, ' ⏎ ')
    case 'dialogue': {
      const parts = [roleLabel('character')]
      if (s('emotion')) parts.push(s('emotion'))
      parts.push(s('text'))
      return parts.join(' · ')
    }
    case 'ai_dialogue': {
      // 与 dialogue 一致：摘要前缀显示对应角色名（MAIN 绑定名 / NPC 名字）
      const parts = [roleLabel('character')]
      parts.push(s('prompt') || '（无提示 · 纯靠上下文生成）')
      return parts.join(' · ')
    }
    case 'free_dialogue': {
      const parts = [roleLabel('character'), s('hint')]
      if (s('end_line')) parts.push(`结束语「${s('end_line')}」`)
      return parts.filter(Boolean).join(' · ')
    }
    case 'choices': {
      const opts = Array.isArray(e.options) ? (e.options as Record<string, unknown>[]) : []
      return opts.map((o) => (typeof o.text === 'string' ? o.text : '（兜底）')).join('　/　')
    }
    case 'input':
      return s('hint') || '请输入...'
    case 'set_variable': {
      const opts = Array.isArray(e.options) ? (e.options as Record<string, unknown>[]) : []
      return opts
        .flatMap((o) =>
          Array.isArray(o.actions)
            ? (o.actions as Record<string, unknown>[]).map((a) =>
                typeof a.content === 'string' ? a.content : '',
              )
            : [],
        )
        .filter(Boolean)
        .join('；')
    }
    case 'chapter_end': {
      const et = s('end_type') || 'linear'
      if (et === 'linear') {
        const target = s('next') || s('next_chapter')
        return target === 'end' || target === '' ? '→ 剧本结束' : `→ ${target}`
      }
      const opts = Array.isArray(e.options) ? (e.options as Record<string, unknown>[]) : []
      return opts
        .map((o) => {
          const cond = typeof o.condition === 'string' ? o.condition : '默认'
          const next = typeof o.next === 'string' ? o.next : '?'
          return `${cond} → ${next}`
        })
        .join('　/　')
    }
    case 'modify_character': {
      const act = s('action')
      const label = act === 'show_character' ? '出场' : act === 'hide_character' ? '退场' : act
      return [roleLabel('character'), label, s('emotion')].filter(Boolean).join(' · ')
    }
    case 'background':
    case 'present_pic':
      return s('imagePath')
    case 'background_effect':
      return s('effect')
    case 'music':
      return s('musicPath')
    case 'sound':
      return s('soundPath')
    case 'ambient':
      return e.stop === true ? `停止 ${s('ambientPath') || '全部轨道'}` : s('ambientPath')
    default:
      return ''
  }
}

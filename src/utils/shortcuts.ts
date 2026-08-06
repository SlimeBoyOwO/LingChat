/**
 * 剧本编辑器快捷键定义与匹配。
 *
 * 默认键位刻意不含 Command（⌘）：跨平台一致用 Ctrl；macOS 用户可在
 * 快捷键面板里自定义为 ⌘ 组合。Redo 默认 Ctrl+Y（原 Ctrl+Shift+Z 仅作
 * 历史兼容不再展示）。
 *
 * 匹配规则：修饰键严格比对（绑定 Ctrl 的事件必须按 Ctrl，未绑定的修饰键
 * 不得按下），方向键上下成对匹配（moveCursor/moveEvent 一次绑定管两个方向）。
 */

/** 单个键位绑定：key 为小写主键（'s'/'enter'/'delete'/'arrowup'/' '/…） */
export interface ShortcutBinding {
  key: string
  ctrl?: boolean
  alt?: boolean
  shift?: boolean
  meta?: boolean
}

export type ShortcutAction =
  | 'save'
  | 'undo'
  | 'redo'
  | 'copyEvent'
  | 'playtest'
  | 'deleteEvent'
  | 'moveCursor'
  | 'moveEvent'
  | 'esc'
  | 'shortcutHelp'
  | 'expandProps'

/** 面板展示顺序即此数组顺序 */
export const SHORTCUT_ACTIONS: ShortcutAction[] = [
  'save',
  'undo',
  'redo',
  'copyEvent',
  'playtest',
  'deleteEvent',
  'moveCursor',
  'moveEvent',
  'esc',
  'shortcutHelp',
  'expandProps',
]

export const DEFAULT_SHORTCUTS: Record<ShortcutAction, ShortcutBinding> = {
  save: { key: 's', ctrl: true },
  undo: { key: 'z', ctrl: true },
  redo: { key: 'y', ctrl: true },
  copyEvent: { key: 'd', ctrl: true },
  playtest: { key: 'enter', ctrl: true },
  deleteEvent: { key: 'delete' },
  moveCursor: { key: 'arrowup' },
  moveEvent: { key: 'arrowup', alt: true },
  esc: { key: 'escape' },
  shortcutHelp: { key: '?' },
  expandProps: { key: 'e', ctrl: true },
}

const isDirKey = (key: string) => key === 'arrowup' || key === 'arrowdown'

/** 绑定是否匹配事件：修饰键严格比对，方向键成对，'?' 由 Shift+/ 产生 */
export const bindingMatches = (b: ShortcutBinding, e: KeyboardEvent): boolean => {
  if (!!b.ctrl !== e.ctrlKey) return false
  if (!!b.alt !== e.altKey) return false
  // '?' 字符本身就是 Shift+/ 的产物，物理按键必然带着 shift，这里忽略 shift 检查
  const questionMark = b.key === '?' || e.key === '?'
  if (!questionMark && !!b.shift !== e.shiftKey) return false
  if (!!b.meta !== e.metaKey) return false
  const k = e.key.toLowerCase()
  if (isDirKey(b.key)) return k === 'arrowup' || k === 'arrowdown'
  return k === b.key.toLowerCase()
}

/** 两个绑定是否视为同一组合（冲突检测用，方向键成对） */
export const bindingsEqual = (a: ShortcutBinding, b: ShortcutBinding): boolean => {
  if (!!a.ctrl !== !!b.ctrl || !!a.alt !== !!b.alt) return false
  if (!!a.shift !== !!b.shift || !!a.meta !== !!b.meta) return false
  if (isDirKey(a.key) && isDirKey(b.key)) return true
  return a.key.toLowerCase() === b.key.toLowerCase()
}

const KEY_LABELS: Record<string, string> = {
  ' ': 'Space',
  enter: 'Enter',
  delete: 'Delete',
  backspace: 'Backspace',
  escape: 'Esc',
  arrowup: '↑ / ↓',
  arrowdown: '↑ / ↓',
}

/** 键位显示文本（Ctrl + S / ⌘ + Y / ↑ / ↓ 等） */
export const formatBinding = (b: ShortcutBinding): string => {
  const mods: string[] = []
  if (b.ctrl) mods.push('Ctrl')
  if (b.meta) mods.push('⌘')
  if (b.alt) mods.push('Alt')
  if (b.shift) mods.push('Shift')
  const key = KEY_LABELS[b.key] ?? b.key.toUpperCase()
  return [...mods, key].join(' + ')
}

/**
 * 捕获模式的按键解析结果。
 * - bind：普通键（含当时按下的修饰键）→ 完成绑定
 * - ignore：纯修饰键（Ctrl/Alt/Shift/Meta）→ 组合键进行中，继续等待
 * - cancel：Esc → 用户取消捕获
 * - blocked：无任何修饰键的普通字符键（字母/数字/空格）→ 拒绝，
 *   这类键在编辑器里承担输入用途，误绑会让组合键失效（如把 Ctrl+S
 *   绑成单独的 S，之后 Ctrl+S 严格匹配失败、S 又误触保存）
 */
export type CaptureResult =
  | { kind: 'bind'; binding: ShortcutBinding }
  | { kind: 'ignore' }
  | { kind: 'cancel' }
  | { kind: 'blocked' }

const isPlainCharKey = (key: string) => /^[a-z0-9 ]$/.test(key)

export const captureFromEvent = (e: KeyboardEvent): CaptureResult => {
  if (e.key === 'Escape') return { kind: 'cancel' }
  if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) return { kind: 'ignore' }
  const binding: ShortcutBinding = {
    key: e.key.toLowerCase(),
    ctrl: e.ctrlKey || undefined,
    // '?' 由 Shift+/ 产生，shift 修饰不单独记录（见 bindingMatches）
    alt: e.altKey || undefined,
    shift: e.key === '?' ? undefined : e.shiftKey || undefined,
    meta: e.metaKey || undefined,
  }
  if (
    isPlainCharKey(binding.key) &&
    !binding.ctrl &&
    !binding.alt &&
    !binding.shift &&
    !binding.meta
  ) {
    return { kind: 'blocked' }
  }
  return { kind: 'bind', binding }
}

/** 绑定数据形状校验（持久化数据可能被旧版本写坏，非法项回退默认） */
export const isValidBinding = (b: unknown): b is ShortcutBinding =>
  !!b &&
  typeof b === 'object' &&
  typeof (b as ShortcutBinding).key === 'string' &&
  (b as ShortcutBinding).key.length > 0 &&
  !['Control', 'Alt', 'Shift', 'Meta', 'Escape'].includes((b as ShortcutBinding).key)

/** 整表校验：非法项用默认键位补齐，合法项保留（用户自定义不被重置） */
export const sanitizeShortcuts = (
  rec: Partial<Record<ShortcutAction, ShortcutBinding>>,
): Record<ShortcutAction, ShortcutBinding> => {
  const out = { ...DEFAULT_SHORTCUTS }
  for (const a of SHORTCUT_ACTIONS) {
    const b = rec[a]
    if (isValidBinding(b)) out[a] = b
  }
  return out
}

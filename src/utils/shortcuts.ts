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
  expandProps: { key: ' ', ctrl: true },
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
 * 从键盘事件解析绑定（捕获模式用）。返回 null 表示取消（Esc 或纯修饰键）。
 * 注意：解析结果包含按下时的全部修饰键，用户按 ⌘+S 就会得到 meta:true。
 */
export const bindingFromEvent = (e: KeyboardEvent): ShortcutBinding | null => {
  if (e.key === 'Escape') return null
  if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) return null
  return {
    key: e.key.toLowerCase(),
    ctrl: e.ctrlKey || undefined,
    // '?' 由 Shift+/ 产生，shift 修饰不单独记录（见 bindingMatches）
    alt: e.altKey || undefined,
    shift: e.key === '?' ? undefined : e.shiftKey || undefined,
    meta: e.metaKey || undefined,
  }
}

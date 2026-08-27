/**
 * 逐字符淡入+上浮渲染器。
 *
 * TypeWriter 的 writeFn 每次 tick 收到的是「当前累积的完整字符串」，
 * 而不是新增的单个字符。因此这里用闭包追踪上一次渲染的文本，
 * 每次只为新增的字符追加动画 span；已渲染的字符节点保持不动，
 * 这样它们的 CSS 动画完成后停留在最终状态，不会被重建重播。
 */

export interface CharRevealOptions {
  /**
   * 生成单个字符的 HTML。
   * @param char    当前字符（可能是 '\n'，返回 '<br>'）
   * @param index   字符在 rawText 中的全局下标（供按换行分色等逻辑）
   * @param rawText 本次完整文本
   * @param animate 是否带淡入+上浮动画：true=单字符 tick；false=批量补全 / 瞬时渲染
   */
  charHtml: (char: string, index: number, rawText: string, animate: boolean) => string
}

export interface CharRevealWriter {
  /** 供 TypeWriter 使用的 writeFn：增量追加动画 span */
  writeFn: (element: HTMLElement, text: string) => void
  /** 立即渲染完整文本（不带动画；用于挂载恢复 / 跳到末尾） */
  renderInstant: (element: HTMLElement, text: string) => void
  /** 重置增量状态。新台词开始前配合清空元素调用，保证首个 tick 判定为全新开始 */
  reset: () => void
}

export function createCharRevealWriter(options: CharRevealOptions): CharRevealWriter {
  // 上一次已渲染到元素里的原文
  let prev = ''

  // 从 fromIndex 起生成 rawText 的 HTML（下标用全局 index，保证分色正确）
  const buildHtml = (rawText: string, fromIndex: number, animate: boolean): string => {
    let html = ''
    for (let i = fromIndex; i < rawText.length; i++) {
      html += options.charHtml(rawText.charAt(i), i, rawText, animate)
    }
    return html
  }

  const writeFn = (element: HTMLElement, text: string): void => {
    if (text === '') {
      // TypeWriter.clear()
      element.innerHTML = ''
      prev = ''
      return
    }

    // 全新开始：start() 不会先调用 writeFn('')，旧行的 span 仍留在元素里。
    // 当新文本不再以 prev 开头（或 prev 为空）时视为新台词，清空后从 0 渲染。
    if (prev === '' || !text.startsWith(prev)) {
      element.innerHTML = ''
      prev = ''
    }

    const addedLen = text.length - prev.length
    if (addedLen > 0) {
      // 只插入新增部分：不能用 innerHTML +=（会重建旧节点并重播动画）。
      // 单字符 tick → 动画；批量（finish 补全剩余字符）→ 瞬时。
      element.insertAdjacentHTML('beforeend', buildHtml(text, prev.length, addedLen === 1))
      prev = text
    }
  }

  const renderInstant = (element: HTMLElement, text: string): void => {
    element.innerHTML = buildHtml(text, 0, false)
    prev = text
  }

  const reset = (): void => {
    prev = ''
  }

  return { writeFn, renderInstant, reset }
}

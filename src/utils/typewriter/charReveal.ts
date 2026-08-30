/**
 * 逐字符淡入+上浮渲染器。
 *
 * TypeWriter 的 writeFn 每次 tick 收到的是「当前累积的完整字符串」，
 * 而不是新增的单个字符。因此这里用闭包追踪上一次渲染的文本，
 * 每次只为新增的字符追加动画 span；已渲染的字符节点保持不动，
 * 这样它们的 CSS 动画完成后停留在最终状态，不会被重建重播。
 *
 * 支持可选的 route 回调：台词合并场景下显示区拆成「台词区/动作区」两个子容器，
 * route 按字符的全局下标决定插到哪个容器；缺省把字符插到传入的 element。
 *
 * 支持可选的 clear 回调：台词合并场景下清空显示必须只清两个子容器的内容，
 * 不能清外层（外层 innerHTML='' 会销毁子容器节点，模板 ref 会指向脱离文档的
 * 旧节点，之后所有字符都插进不可见处）。缺省清空整个 element（无子容器的单元素场景）。
 */

export interface CharRevealOptions {
  /**
   * 生成单个字符的 HTML。
   * @param char    当前字符（可能是 '\n'，返回 '<br>'）
   * @param index   字符在 rawText 中的全局下标
   * @param rawText 本次完整文本
   * @param animate 是否带淡入+上浮动画：true=单字符 tick；false=批量补全 / 瞬时渲染
   */
  charHtml: (char: string, index: number, rawText: string, animate: boolean) => string;
  /**
   * 可选：字符插入目标。返回 null/undefined 时退化为 element。
   * 用于把不同片段(台词/动作)的字符路由到不同子容器。
   * @param index   字符在 rawText 中的全局下标
   * @param rawText 本次完整文本
   */
  route?: (index: number, rawText: string) => HTMLElement | null | undefined;
  /**
   * 可选：清空显示内容。缺省清空整个 element（element.innerHTML = ''）。
   * 有多级容器时必须只清内容容器、保留容器结构（见文件头注释）。
   */
  clear?: (element: HTMLElement) => void;
}

export interface CharRevealWriter {
  /** 供 TypeWriter 使用的 writeFn：增量追加动画 span */
  writeFn: (element: HTMLElement, text: string) => void;
  /** 立即渲染完整文本（不带动画；用于挂载恢复 / 跳到末尾） */
  renderInstant: (element: HTMLElement, text: string) => void;
  /** 重置增量状态。新台词开始前配合清空元素调用，保证首个 tick 判定为全新开始 */
  reset: () => void;
  /** 清空显示内容（不重置增量状态） */
  clear: (element: HTMLElement) => void;
}

export function createCharRevealWriter(options: CharRevealOptions): CharRevealWriter {
  // 上一次已渲染到元素里的原文
  let prev = "";

  // 清空显示：缺省清外层；有自定义 clear（如台词合并的两容器场景）则只清内容容器
  const clearDisplay = (element: HTMLElement): void => {
    if (options.clear) {
      options.clear(element);
    } else {
      element.innerHTML = "";
    }
  };

  // 确定某个字符的插入目标
  const targetFor = (element: HTMLElement, index: number, text: string): HTMLElement =>
    options.route ? (options.route(index, text) ?? element) : element;

  const writeFn = (element: HTMLElement, text: string): void => {
    if (text === "") {
      // TypeWriter.clear() —— 清显示（含子容器）
      clearDisplay(element);
      prev = "";
      return;
    }

    // 全新开始：start() 不会先调用 writeFn('')，旧行的 span 仍留在元素里。
    // 当新文本不再以 prev 开头（或 prev 为空）时视为新台词，清空后从 0 渲染。
    if (prev === "" || !text.startsWith(prev)) {
      clearDisplay(element);
      prev = "";
    }

    const addedLen = text.length - prev.length;
    if (addedLen > 0) {
      // 只插入新增部分：不能用 innerHTML +=（会重建旧节点并重播动画）。
      // 单字符 tick → 动画；批量（finish 补全剩余字符）→ 瞬时。
      // 逐字符按 route 插入对应子容器。
      for (let i = prev.length; i < text.length; i++) {
        const target = targetFor(element, i, text);
        target.insertAdjacentHTML(
          "beforeend",
          options.charHtml(text.charAt(i), i, text, addedLen === 1)
        );
      }
      prev = text;
    }
  };

  const renderInstant = (element: HTMLElement, text: string): void => {
    clearDisplay(element);
    for (let i = 0; i < text.length; i++) {
      const target = targetFor(element, i, text);
      target.insertAdjacentHTML("beforeend", options.charHtml(text.charAt(i), i, text, false));
    }
    prev = text;
  };

  const reset = (): void => {
    prev = "";
  };

  const clear = (element: HTMLElement): void => {
    clearDisplay(element);
    prev = "";
  };

  return { writeFn, renderInstant, reset, clear };
}

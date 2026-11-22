const HIGHLIGHT_CLASS = "settings-jump-highlight";
const HIGHLIGHT_DURATION_MS = 2200;
const DEFAULT_TIMEOUT_MS = 800;
const POLL_INTERVAL_MS = 60;

let activeClearTimer: number | null = null;
let activeFocusSeq = 0;
let cancelActiveWait: (() => void) | null = null;

export function escapeAttrValue(value: string): string {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(value);
  }
  return value.replace(/["\\]/g, "\\$&");
}

function clearExistingHighlights(): void {
  if (activeClearTimer !== null) {
    window.clearTimeout(activeClearTimer);
    activeClearTimer = null;
  }
  document.querySelectorAll(`.${HIGHLIGHT_CLASS}`).forEach((node) => {
    node.classList.remove(HIGHLIGHT_CLASS);
  });
}

function applyHighlight(el: HTMLElement): void {
  clearExistingHighlights();
  el.scrollIntoView({ behavior: "smooth", block: "center", inline: "nearest" });
  el.classList.remove(HIGHLIGHT_CLASS);
  // 强制重排以重播脉冲动画
  void el.offsetWidth;
  el.classList.add(HIGHLIGHT_CLASS);
  activeClearTimer = window.setTimeout(() => {
    el.classList.remove(HIGHLIGHT_CLASS);
    activeClearTimer = null;
  }, HIGHLIGHT_DURATION_MS);
}

function findVisibleElement(selector: string): HTMLElement | null {
  const nodes = document.querySelectorAll<HTMLElement>(selector);
  for (const el of nodes) {
    if (el.getClientRects().length > 0) return el;
  }
  return null;
}

export function cancelPendingFocus(): void {
  activeFocusSeq++;
  cancelActiveWait?.();
  cancelActiveWait = null;
}

export function focusElementBySelector(
  selector: string,
  options?: {
    fallbackSelector?: string;
    timeoutMs?: number;
  },
): Promise<boolean> {
  cancelPendingFocus();
  const seq = activeFocusSeq;
  const timeoutMs = Math.max(0, options?.timeoutMs ?? DEFAULT_TIMEOUT_MS);
  const fallbackSelector = options?.fallbackSelector;

  return new Promise<boolean>((resolve) => {
    let settled = false;
    let rafId: number | null = null;
    let pollTimer: number | null = null;
    let timeoutTimer: number | null = null;
    let observer: MutationObserver | null = null;

    const cleanup = () => {
      if (rafId !== null) {
        window.cancelAnimationFrame(rafId);
        rafId = null;
      }
      if (pollTimer !== null) {
        window.clearInterval(pollTimer);
        pollTimer = null;
      }
      if (timeoutTimer !== null) {
        window.clearTimeout(timeoutTimer);
        timeoutTimer = null;
      }
      if (observer) {
        observer.disconnect();
        observer = null;
      }
      if (cancelActiveWait === cancel) {
        cancelActiveWait = null;
      }
    };

    const finish = (matched: boolean) => {
      if (settled) return;
      settled = true;
      cleanup();
      resolve(matched);
    };

    const cancel = () => finish(false);
    cancelActiveWait = cancel;

    const tryFocus = (allowFallback: boolean): boolean => {
      if (settled || seq !== activeFocusSeq) {
        finish(false);
        return true;
      }
      const direct = findVisibleElement(selector);
      if (direct) {
        applyHighlight(direct);
        finish(true);
        return true;
      }
      if (allowFallback && fallbackSelector) {
        const fallback = findVisibleElement(fallbackSelector);
        if (fallback) {
          applyHighlight(fallback);
          finish(true);
          return true;
        }
      }
      return false;
    };

    const scheduleCheck = () => {
      if (settled || rafId !== null) return;
      rafId = window.requestAnimationFrame(() => {
        rafId = null;
        tryFocus(false);
      });
    };

    // 等待一帧确保刚切换的 v-if/v-show 容器完成布局后再滚动高亮
    rafId = window.requestAnimationFrame(() => {
      rafId = null;
      if (tryFocus(timeoutMs === 0)) return;
      if (timeoutMs === 0) {
        finish(false);
        return;
      }

      if (typeof MutationObserver !== "undefined" && document.body) {
        observer = new MutationObserver(scheduleCheck);
        observer.observe(document.body, {
          childList: true,
          subtree: true,
          attributes: true,
          attributeFilter: [
            "style",
            "class",
            "hidden",
            "open",
            "data-setting-anchor",
            "data-setting-key",
          ],
        });
      }

      pollTimer = window.setInterval(() => {
        tryFocus(false);
      }, POLL_INTERVAL_MS);

      timeoutTimer = window.setTimeout(() => {
        if (!tryFocus(true)) {
          finish(false);
        }
      }, timeoutMs);
    });
  });
}

export function focusSettingAnchor(
  anchor: string,
  options?: { fallbackAnchor?: string; timeoutMs?: number },
): Promise<boolean> {
  const selector = `[data-setting-anchor="${escapeAttrValue(anchor)}"]`;
  const fallbackSelector = options?.fallbackAnchor
    ? `[data-setting-anchor="${escapeAttrValue(options.fallbackAnchor)}"]`
    : undefined;
  return focusElementBySelector(selector, {
    fallbackSelector,
    timeoutMs: options?.timeoutMs,
  });
}

export function focusSettingKey(
  settingKey: string,
  options?: { fallbackAnchor?: string; timeoutMs?: number },
): Promise<boolean> {
  const selector = `[data-setting-key="${escapeAttrValue(settingKey)}"]`;
  const fallbackSelector = options?.fallbackAnchor
    ? `[data-setting-anchor="${escapeAttrValue(options.fallbackAnchor)}"]`
    : undefined;
  return focusElementBySelector(selector, {
    fallbackSelector,
    timeoutMs: options?.timeoutMs,
  });
}

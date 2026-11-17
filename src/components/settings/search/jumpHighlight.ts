import { nextTick, onActivated, onDeactivated, onUnmounted, watch } from "vue";
import { useUIStore, type SettingsJumpTarget } from "@/stores/modules/ui/ui";

const HIGHLIGHT_CLASS = "settings-jump-highlight";
const HIGHLIGHT_DURATION_MS = 2200;
const CROSS_TAB_SLIDE_DELAY_MS = 340;

let activeClearTimer: number | null = null;
let activeSearchSeq = 0;

/** 安全转义 CSS 属性选择器值 */
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
  // 强制重排，确保连续两次点击同一目标也能重播脉冲动画
  void el.offsetWidth;
  el.classList.add(HIGHLIGHT_CLASS);
  activeClearTimer = window.setTimeout(() => {
    el.classList.remove(HIGHLIGHT_CLASS);
    activeClearTimer = null;
  }, HIGHLIGHT_DURATION_MS);
}

function findVisibleElement(selector: string): HTMLElement | null {
  const el = document.querySelector<HTMLElement>(selector);
  if (!el) return null;
  return el.getClientRects().length > 0 ? el : null;
}

/**
 * 等待目标元素出现在可视 DOM 中，滚动到视口中央并高亮闪烁。
 */
export async function focusElementBySelector(
  selector: string,
  options?: {
    delayMs?: number;
    retries?: number;
    fallbackSelector?: string;
  },
): Promise<boolean> {
  const seq = ++activeSearchSeq;
  const delayMs = options?.delayMs ?? 0;
  const maxRetries = options?.retries ?? 16;

  await nextTick();
  if (delayMs > 0) {
    await new Promise<void>((resolve) => window.setTimeout(resolve, delayMs));
  }
  if (seq !== activeSearchSeq) return false;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    if (seq !== activeSearchSeq) return false;
    const direct = findVisibleElement(selector);
    if (direct) {
      applyHighlight(direct);
      return true;
    }
    if (attempt >= maxRetries - 2 && options?.fallbackSelector) {
      const fallback = findVisibleElement(options.fallbackSelector);
      if (fallback) {
        applyHighlight(fallback);
        return true;
      }
    }
    await new Promise<void>((resolve) => window.setTimeout(resolve, 40));
  }
  return false;
}

/** 按 data-setting-anchor 属性滚动并高亮目标卡片/区块 */
export function focusSettingAnchor(
  anchor: string,
  options?: { crossTab?: boolean; fallbackAnchor?: string },
): Promise<boolean> {
  const selector = `[data-setting-anchor="${escapeAttrValue(anchor)}"]`;
  const fallbackSelector = options?.fallbackAnchor
    ? `[data-setting-anchor="${escapeAttrValue(options.fallbackAnchor)}"]`
    : undefined;
  return focusElementBySelector(selector, {
    delayMs: options?.crossTab ? CROSS_TAB_SLIDE_DELAY_MS : 0,
    fallbackSelector,
  });
}

/**
 * 在设置页面内消费 uiStore.pendingSettingsJump。
 * 兼容 KeepAlive 缓存切换与直接挂载，避免在停用状态下误消费跳转意图。
 */
export function useSettingsJumpConsumer(
  matches: (jump: SettingsJumpTarget) => boolean,
  onConsume?: (jump: SettingsJumpTarget) => void | Promise<void>,
): void {
  const uiStore = useUIStore();
  let isActive = true;
  let lastHandledNonce: number | undefined;

  const tryConsume = async () => {
    if (!isActive) return;
    const jump = uiStore.pendingSettingsJump;
    if (!jump || !matches(jump)) return;
    if (jump.nonce !== undefined && jump.nonce === lastHandledNonce) return;
    lastHandledNonce = jump.nonce;
    // 先取副本再清 store，避免异步执行期间状态被提前覆盖
    const snapshot: SettingsJumpTarget = { ...jump };
    uiStore.clearPendingSettingsJump();
    await nextTick();
    if (onConsume) {
      await onConsume(snapshot);
    } else if (snapshot.anchor) {
      await focusSettingAnchor(snapshot.anchor, { crossTab: snapshot.crossTab });
    }
  };

  onActivated(() => {
    isActive = true;
    void tryConsume();
  });

  onDeactivated(() => {
    isActive = false;
  });

  onUnmounted(() => {
    isActive = false;
  });

  watch(
    () => uiStore.pendingSettingsJump?.nonce,
    () => {
      void tryConsume();
    },
    { immediate: true, flush: "post" },
  );
}

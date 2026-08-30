import { nextTick } from "vue";

/**
 * 快照前隐藏辅助 — 仅 Windows 快照期间使用
 *
 * 通过 `visibility:hidden` 隐藏指定元素（保留占位，避免布局塌陷），
 * 等待 `nextTick + 双 rAF` 确保 Chromium 完成重绘后再触发截屏，
 * 截屏完成后 `finally restore` 恢复可见性。
 *
 * 抽为 composable 供 MainMenu（主菜单选项）与 MainChat（#menu-panel）复用。
 */
export function useHideForSnapshot() {
  function resolveEl(raw: unknown): HTMLElement | null {
    if (!raw) return null;
    if (raw instanceof HTMLElement) return raw;
    const maybe = raw as { $el?: HTMLElement };
    if (maybe?.$el instanceof HTMLElement) return maybe.$el;
    return null;
  }

  async function hide(el: HTMLElement | null): Promise<void> {
    if (!el) return;
    el.style.visibility = "hidden";
    await nextTick();
    // 双 rAF 确保重绘完成再截，避免高刷屏抢拍
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
  }

  function restore(el: HTMLElement | null): void {
    if (!el || !(el instanceof HTMLElement)) return;
    el.style.visibility = "";
  }

  return { hide, restore, resolveEl };
}

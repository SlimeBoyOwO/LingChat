import {
  getCurrentInstance,
  inject,
  nextTick,
  onDeactivated,
  onUnmounted,
  provide,
  watch,
  type InjectionKey,
  type Ref,
} from "vue";
import { cancelPendingFocus, focusSettingAnchor, focusSettingKey } from "./jumpHighlight";

export type AdvanceTab = "menu" | "llm" | "tts" | "asr" | "tools" | "cast" | "other" | "memory";

export interface AdvanceJumpTarget {
  advanceTab: AdvanceTab;
  anchor?: string;
  fallbackAnchor?: string;
  category?: string;
  subcategory?: string;
  settingKey?: string;
  toolGroup?: string;
  openToolSubpage?: boolean;
}

export type JumpRevealer = (target: AdvanceJumpTarget) => void | Promise<void>;

export interface AdvanceJumpContext {
  jumpTo: (target: AdvanceJumpTarget) => Promise<void>;
  registerRevealer: (tab: AdvanceTab, fn: JumpRevealer) => () => void;
}

const ADVANCE_JUMP_KEY: InjectionKey<AdvanceJumpContext> = Symbol("advanceJump");

export function provideAdvanceJump(
  advanceTab: Ref<string>,
  onBeforeJump?: (target: AdvanceJumpTarget) => void | Promise<void>,
): AdvanceJumpContext {
  const revealers = new Map<AdvanceTab, JumpRevealer>();
  let jumpSeq = 0;
  let disposed = false;
  let internalTabSwitch = false;

  const cancelPendingJump = () => {
    jumpSeq++;
    cancelPendingFocus();
  };

  watch(
    advanceTab,
    () => {
      if (!internalTabSwitch) {
        cancelPendingJump();
      }
    },
    { flush: "sync" },
  );

  const registerRevealer = (tab: AdvanceTab, fn: JumpRevealer): (() => void) => {
    revealers.set(tab, fn);
    return () => {
      if (revealers.get(tab) === fn) {
        revealers.delete(tab);
      }
    };
  };

  const jumpTo = async (target: AdvanceJumpTarget): Promise<void> => {
    const seq = ++jumpSeq;
    cancelPendingFocus();

    await onBeforeJump?.(target);
    if (disposed || seq !== jumpSeq) return;

    internalTabSwitch = true;
    try {
      advanceTab.value = target.advanceTab;
    } finally {
      internalTabSwitch = false;
    }
    await nextTick();
    if (disposed || seq !== jumpSeq) return;

    const revealer = revealers.get(target.advanceTab);
    if (revealer) {
      try {
        await revealer(target);
      } catch (err) {
        console.error("[advanceJump] revealer failed:", err);
      }
      if (disposed || seq !== jumpSeq || revealers.get(target.advanceTab) !== revealer) return;
      await nextTick();
      if (disposed || seq !== jumpSeq || revealers.get(target.advanceTab) !== revealer) return;
    }

    if (target.settingKey) {
      await focusSettingKey(target.settingKey, {
        fallbackAnchor: target.fallbackAnchor || target.anchor,
      });
    } else if (target.anchor) {
      await focusSettingAnchor(target.anchor, {
        fallbackAnchor: target.fallbackAnchor,
      });
    }
  };

  if (getCurrentInstance()) {
    onDeactivated(cancelPendingJump);

    onUnmounted(() => {
      disposed = true;
      revealers.clear();
      cancelPendingJump();
    });
  }

  const ctx: AdvanceJumpContext = {
    jumpTo,
    registerRevealer,
  };
  provide(ADVANCE_JUMP_KEY, ctx);
  return ctx;
}

export function useAdvanceJump(): AdvanceJumpContext {
  const ctx = inject(ADVANCE_JUMP_KEY, null);
  if (!ctx) {
    throw new Error("useAdvanceJump() must be used inside a component under provideAdvanceJump()");
  }
  return ctx;
}

export function useJumpRevealer(tab: AdvanceTab, fn: JumpRevealer): void {
  const ctx = inject(ADVANCE_JUMP_KEY, null);
  if (!ctx) return;
  const unregister = ctx.registerRevealer(tab, fn);
  if (getCurrentInstance()) {
    onUnmounted(unregister);
  }
}

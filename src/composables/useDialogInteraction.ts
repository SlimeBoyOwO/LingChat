/**
 * 对话框交互 composable
 * 隐藏状态、滚轮查看历史、空格键快速隐藏
 */
import { ref, onMounted, onUnmounted } from "vue";

export interface UseDialogInteractionOptions {
  /** 打开历史记录面板的回调 */
  openHistory: () => void;
}

export function useDialogInteraction(options: UseDialogInteractionOptions) {
  // ── 对话框隐藏状态 ──
  const isHidden = ref(false);

  /** 隐藏对话框 */
  function hide() {
    isHidden.value = true;
  }

  // ── 滚轮查看历史记录 ──
  function handleWheelHistory(e: WheelEvent) {
    // 向上滚动 (deltaY < 0) 打开历史面板
    if (e.deltaY < -10) {
      options.openHistory();
    }
  }

  // ── 空格键隐藏/显示对话框 ──
  function handleKeydown(e: KeyboardEvent) {
    // 在输入框中不触发
    const target = e.target as HTMLElement;
    if (
      target &&
      (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)
    ) {
      return;
    }
    if (e.code === "Space") {
      e.preventDefault();
      isHidden.value = !isHidden.value;
    }
  }

  // ── 生命周期：绑定键盘事件 ──
  onMounted(() => {
    document.addEventListener("keydown", handleKeydown);
  });

  onUnmounted(() => {
    document.removeEventListener("keydown", handleKeydown);
  });

  return {
    isHidden,
    hide,
    handleWheelHistory,
  };
}

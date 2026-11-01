import { onUnmounted, reactive } from "vue";

export interface TransientStatus {
  message: string;
  color: string;
}

/** 短暂显示操作结果；重复调用会重置计时，组件卸载时自动清理。 */
export function useTransientStatus(timeoutMs = 5000, defaultColor = "#4ade80") {
  const status = reactive<TransientStatus>({ message: "", color: defaultColor });
  let timer: ReturnType<typeof setTimeout> | null = null;

  function clear() {
    if (timer === null) return;
    clearTimeout(timer);
    timer = null;
  }

  function show(message: string, color = defaultColor) {
    clear();
    status.message = message;
    status.color = color;
    timer = setTimeout(() => {
      status.message = "";
      timer = null;
    }, timeoutMs);
  }

  onUnmounted(clear);

  return { status, show, clear };
}

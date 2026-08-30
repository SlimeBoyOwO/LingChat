import { ref, computed, onUnmounted } from "vue";
import type { Ref, ComputedRef } from "vue";
import { TypeWriter } from "../../utils/typewriter/TypeWriter";
import type { TypeWriterStatus } from "../../utils/typewriter/TypeWriter";

export function useTypeWriter(
  elementRef: Ref<HTMLElement | null>,
  onTextUpdate?: (text: string) => void,
  writeFn?: (element: HTMLElement, text: string) => void
) {
  const typeWriter = ref<TypeWriter | null>(null);
  const localStatus = ref<TypeWriterStatus>("idle");

  /** Reactive computed: true while the typewriter is actively typing. */
  const isTyping = computed(() => localStatus.value === "typing");

  /** Reactive computed: current typewriter state. */
  const status: ComputedRef<TypeWriterStatus> = computed(() => localStatus.value);

  const init = () => {
    if (elementRef.value && !typeWriter.value) {
      typeWriter.value = new TypeWriter(elementRef.value, onTextUpdate, undefined, writeFn);
      // 台词合并续打：append() 复用原 start() 的打字循环，原 promise 早已 resolve，
      // 结束信号只能靠 onFinish 同步（自然完成 / 点击跳过都会触发）。
      typeWriter.value.onFinish(() => {
        localStatus.value = "completed";
      });
    }
  };

  /**
   * Start the typewriter animation. Returns a Promise that resolves
   * when typing completes naturally or is cancelled.
   */
  const startTyping = async (text: string, speed?: number): Promise<void> => {
    if (!typeWriter.value) init();
    if (!typeWriter.value) {
      console.warn("[useTypeWriter] Cannot start: element ref is null");
      return;
    }
    localStatus.value = "typing";
    await typeWriter.value.start(text, speed);
    // Sync with the instance's actual status after completion
    localStatus.value = typeWriter.value.status;
  };

  /** Stop the typewriter animation and clear displayed text. */
  const stopTyping = () => {
    typeWriter.value?.stop();
    typeWriter.value?.clear();
    localStatus.value = "idle";
  };

  /** Immediately complete the current animation (show full text). */
  const finishTyping = () => {
    typeWriter.value?.finish();
    localStatus.value = typeWriter.value?.status ?? "completed";
  };

  /**
   * 追加文本续打（台词合并：i+1 句到达后接续到当前打字目标末尾）。
   * 不等新 promise——原 start() 的 promise 已 resolve，续打完成靠 onFinish 同步。
   */
  const appendTyping = (text: string) => {
    if (!typeWriter.value) return;
    typeWriter.value.append(text);
    localStatus.value = typeWriter.value.status;
  };

  onUnmounted(() => {
    typeWriter.value?.destroy();
    typeWriter.value = null;
  });

  return {
    startTyping,
    stopTyping,
    finishTyping,
    appendTyping,
    isTyping,
    status,
  };
}

import { ref, onUnmounted, Ref } from "vue";
import { TypeWriter } from "../../utils/typewriter/TypeWriter";

export function useTypeWriter(
  elementRef: Ref<HTMLInputElement | HTMLTextAreaElement | null>
) {
  const typeWriter = ref<TypeWriter | null>(null);
  const isTyping = ref(false);

  const init = () => {
    if (elementRef.value) {
      typeWriter.value = new TypeWriter(elementRef.value);
    }
  };

  const startTyping = async (text: string, speed?: number) => {
    if (!typeWriter.value) init();
    isTyping.value = true;
    await typeWriter.value?.start(text, speed);
    isTyping.value = false;
  };

  const stopTyping = () => {
    typeWriter.value?.stop();
    typeWriter.value?.clear();
    isTyping.value = false;
  };

  onUnmounted(() => {
    stopTyping();
  });

  return {
    startTyping,
    stopTyping,
    isTyping,
  };
}

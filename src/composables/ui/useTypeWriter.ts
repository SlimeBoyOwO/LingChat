import { ref, computed, onUnmounted } from 'vue'
import type { Ref, ComputedRef } from 'vue'
import { TypeWriter } from '../../utils/typewriter/TypeWriter'
import type { TypeWriterStatus } from '../../utils/typewriter/TypeWriter'

export function useTypeWriter(
  elementRef: Ref<HTMLInputElement | HTMLTextAreaElement | null>,
  onTextUpdate?: (text: string) => void,
) {
  const typeWriter = ref<TypeWriter | null>(null)
  const localStatus = ref<TypeWriterStatus>('idle')

  /** Reactive computed: true while the typewriter is actively typing. */
  const isTyping = computed(() => localStatus.value === 'typing')

  /** Reactive computed: current typewriter state. */
  const status: ComputedRef<TypeWriterStatus> = computed(() => localStatus.value)

  const init = () => {
    if (elementRef.value && !typeWriter.value) {
      typeWriter.value = new TypeWriter(elementRef.value, onTextUpdate)
    }
  }

  /**
   * Start the typewriter animation. Returns a Promise that resolves
   * when typing completes naturally or is cancelled.
   */
  const startTyping = async (text: string, speed?: number): Promise<void> => {
    if (!typeWriter.value) init()
    if (!typeWriter.value) {
      console.warn('[useTypeWriter] Cannot start: element ref is null')
      return
    }
    localStatus.value = 'typing'
    await typeWriter.value.start(text, speed)
    // Sync with the instance's actual status after completion
    localStatus.value = typeWriter.value.status
  }

  /** Stop the typewriter animation and clear displayed text. */
  const stopTyping = () => {
    typeWriter.value?.stop()
    typeWriter.value?.clear()
    localStatus.value = 'idle'
  }

  /** Immediately complete the current animation (show full text). */
  const finishTyping = () => {
    typeWriter.value?.finish()
    localStatus.value = typeWriter.value?.status ?? 'completed'
  }

  onUnmounted(() => {
    typeWriter.value?.destroy()
    typeWriter.value = null
  })

  return {
    startTyping,
    stopTyping,
    finishTyping,
    isTyping,
    status,
  }
}

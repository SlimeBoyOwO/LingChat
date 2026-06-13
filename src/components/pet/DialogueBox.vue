<template>
  <div
    @click="handleDialogueClick"
    class="relative flex items-center justify-center w-full h-full z-30 cursor-pointer transition-all duration-300 ease-out"
    :class="isVisible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-2 pointer-events-none'"
  >
    <div
      class="relative w-[85%] max-h-[calc(72px*var(--pet-ui-scale,1))] rounded-[calc(20px*var(--pet-ui-scale,1))] px-[calc(18px*var(--pet-ui-scale,1))] py-[calc(6px*var(--pet-ui-scale,1))] text-white backdrop-blur-xl backdrop-saturate-200 border bg-neutral-950/50 border-white/10 transition-all duration-300 hover:bg-neutral-950/65 hover:scale-[1.02] hover:-translate-y-0.2 hover:border-white/20 [text-shadow:0_1px_4px_rgba(0,0,0,0.5)]"
    >
      <div
        class="absolute -bottom-2.5 left-1/2 -translate-x-1/2 w-0 h-0 border-l-10 border-l-transparent border-r-10 border-r-transparent border-t-white/10 drop-shadow-md"
      ></div>
      <div
        class="absolute -bottom-2 left-1/2 -translate-x-1/2 w-0 h-0 border-l-8 border-l-transparent border-r-8 border-r-transparent border-t-8 border-t-white/8"
      ></div>

      <div
        v-if="characterEmotion"
        class="text-[calc(12px*var(--pet-ui-scale,1))] text-cyan-400 font-semibold italic tracking-wider mb-0.5 drop-shadow-[0_1px_4px_rgba(0,176,255,0.5)] truncate"
      >
        {{ characterEmotion }}
      </div>

      <div
        ref="textareaRef"
        class="text-[calc(15px*var(--pet-ui-scale,1))] leading-snug font-medium overflow-y-auto max-h-[calc(40px*var(--pet-ui-scale,1))]"
      ></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useGameStore } from '../../stores/modules/game'
import { eventQueue } from '../../core/events/event-queue'
import { useUIStore } from '../../stores/modules/ui/ui'
import { useTypeWriter } from '../../composables/ui/useTypeWriter'

const gameStore = useGameStore()
const uiStore = useUIStore()

const currentDisplayedText = ref('')

const emit = defineEmits(['player-continued', 'dialog-proceed'])

const isVisible = computed(() => {
  return gameStore.currentStatus === 'responding' && gameStore.currentLine.trim() !== ''
})

const characterEmotion = computed(() => {
  return uiStore.showCharacterEmotion ? uiStore.showCharacterEmotion : ''
})

const handleDialogueClick = () => {
  if (isVisible.value) {
    console.log('点击对话框，继续下一句')
    continueDialog(true)
    eventQueue.continue()
  }
}

const textareaRef = ref<HTMLTextAreaElement | null>(null)

const { startTyping, stopTyping, isTyping } = useTypeWriter(textareaRef, (text) => {
  currentDisplayedText.value = text
})

watch([() => uiStore.showCharacterLine, () => gameStore.currentStatus], ([newLine, newStatus]) => {
  if (newLine && newLine !== '' && newStatus === 'responding') {
    currentDisplayedText.value = ''
    startTyping(newLine, uiStore.typeWriterSpeed)
  } else if (newStatus === 'input') {
    stopTyping()
    currentDisplayedText.value = ''
  }
})

function continueDialog(isPlayerTrigger: boolean): boolean {
  const needWait = eventQueue.continue()
  if (!needWait) {
    if (isPlayerTrigger) emit('player-continued')
    emit('dialog-proceed')
  }

  return needWait
}

defineExpose({
  continueDialog,
  isTyping,
})
</script>

<style scoped></style>

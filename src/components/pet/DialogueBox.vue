<template>
  <div
    @click="handleDialogueClick"
    class="relative flex items-center justify-center w-full z-30 cursor-pointer transition-all duration-300 ease-out"
    :class="isVisible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-2 pointer-events-none'"
  >
    <div
      ref="bubbleRef"
      class="relative w-[85%] rounded-[calc(20px*var(--pet-ui-scale,1))] px-[calc(18px*var(--pet-ui-scale,1))] py-[calc(6px*var(--pet-ui-scale,1))] text-white backdrop-blur-xl backdrop-saturate-200 border bg-neutral-950/50 border-white/10 transition-all duration-300 hover:bg-neutral-950/65 hover:scale-[1.02] hover:-translate-y-0.2 hover:border-white/20 [text-shadow:0_1px_4px_rgba(0,0,0,0.5)]"
      :style="{ maxHeight: `calc(var(--dialog-h) - 20px)` }"
    >
      <div
        class="absolute -bottom-2.5 left-1/2 -translate-x-1/2 w-0 h-0 border-l-10 border-l-transparent border-r-10 border-r-transparent border-t-white/10 drop-shadow-md"
      ></div>
      <div
        class="absolute -bottom-2 left-1/2 -translate-x-1/2 w-0 h-0 border-l-8 border-l-transparent border-r-8 border-r-transparent border-t-8 border-t-white/8"
      ></div>

      <div class="relative overflow-hidden">
        <Transition name="emotion-slide">
          <div
            v-if="characterEmotion"
            :key="characterEmotion"
            class="inline-block max-w-full text-[calc(12px*var(--pet-ui-scale,1))] text-cyan-400 font-semibold italic tracking-wider mb-0.5 drop-shadow-[0_1px_4px_rgba(0,176,255,0.5)] truncate"
          >
            {{ characterEmotion }}
          </div>
        </Transition>
      </div>

      <div
        ref="textareaRef"
        class="dialog-text-lock text-[calc(15px*var(--pet-ui-scale,1))] leading-snug font-medium overflow-y-auto whitespace-pre-line break-all pb-[0.4em] [text-shadow:0_0_3px_rgba(0,0,0,0.9),0_1px_4px_rgba(0,0,0,0.5)]"
        :style="{ maxHeight: `calc(var(--dialog-h) - 52px)` }"
      ></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted } from 'vue'
import { useGameStore } from '../../stores/modules/game'
import { eventQueue } from '../../core/events/event-queue'
import { useUIStore } from '../../stores/modules/ui/ui'
import { useTypeWriter } from '../../composables/ui/useTypeWriter'
import { createCharRevealWriter } from '../../utils/typewriter/charReveal'
import { escapeHtml } from '../../utils/escapeHtml'

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

const textareaRef = ref<HTMLElement | null>(null)
const bubbleRef = ref<HTMLElement | null>(null)

// 逐字符淡入+上浮渲染器（颜色/阴影继承气泡样式）
const charReveal = createCharRevealWriter({
  charHtml: (char, _index, _rawText, animate) => {
    if (char === '\n') return '<br>'
    if (char === ' ') return ' '
    const anim = animate
      ? ';animation:tw-char-rise .28s cubic-bezier(.22, 1, .36, 1) forwards'
      : ''
    return `<span style="display:inline-block${anim}">${escapeHtml(char)}</span>`
  },
})

const { startTyping, stopTyping, isTyping } = useTypeWriter(
  textareaRef,
  (text) => {
    currentDisplayedText.value = text
  },
  // DialogueBox 正文为普通 <div>（非 textarea/input），必须提供 writeFn
  // 逐字符渲染：由 charReveal 增量生成动画 span
  charReveal.writeFn,
)

watch([() => uiStore.showCharacterLine, () => gameStore.currentStatus], ([newLine, newStatus]) => {
  if (newLine && newLine !== '' && newStatus === 'responding') {
    currentDisplayedText.value = ''
    // 清空旧行并重置渲染器增量状态，避免新台词被误判为旧文本的延续
    if (textareaRef.value) {
      textareaRef.value.innerHTML = ''
      charReveal.reset()
      // 锁定最终高度：用离屏克隆同步测量完整文本渲染后的高度（受 maxHeight 钳制），
      // 再把真实容器高度设为该值。打字期间盒子不再随逐字换行而跳动；
      // 行与行之间的高度变化由 .dialog-text-lock 的 height 过渡平滑扩展/收缩。
      const el = textareaRef.value
      const clone = el.cloneNode(false) as HTMLDivElement
      clone.style.position = 'fixed'
      clone.style.left = '-9999px'
      clone.style.top = '0'
      clone.style.visibility = 'hidden'
      clone.style.pointerEvents = 'none'
      clone.style.height = 'auto'
      clone.style.overflowY = 'visible'
      clone.style.width = el.clientWidth + 'px'
      el.parentElement?.appendChild(clone)
      charReveal.renderInstant(clone, newLine)
      const finalH = clone.offsetHeight
      clone.remove()
      charReveal.reset()
      el.style.height = finalH + 'px'
    }
    startTyping(newLine, uiStore.typeWriterSpeed)
  } else if (newStatus === 'input') {
    stopTyping()
    currentDisplayedText.value = ''
    if (textareaRef.value) textareaRef.value.style.height = ''
  }
})

// 模式切换重挂载：立即从 store 恢复当前台词（不重播打字动画）
onMounted(() => {
  const line = uiStore.showCharacterLine
  if (line && line !== '' && gameStore.currentStatus === 'responding' && textareaRef.value) {
    charReveal.renderInstant(textareaRef.value, line)
    currentDisplayedText.value = line
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
  bubbleRef,
})
</script>

<style scoped>
/* 情绪标签切换：上一个情绪向左滑出，下一个情绪从右侧滑入（推挤效果） */
.emotion-slide-enter-active,
.emotion-slide-leave-active {
  transition:
    transform 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94),
    opacity 0.3s ease;
}
/* 离开中的旧情绪脱离文档流，覆盖在新情绪上方向左滑出，
 * 容器宽度由新情绪决定 */
.emotion-slide-leave-active {
  position: absolute;
  left: 0;
  top: 0;
}
.emotion-slide-enter-from {
  transform: translateX(100%);
  opacity: 0;
}
.emotion-slide-leave-to {
  transform: translateX(-100%);
  opacity: 0;
}

/* 打字期间高度锁定为最终高度；行切换时高度平滑扩展/收缩 */
.dialog-text-lock {
  transition: height 0.25s cubic-bezier(0.25, 0.46, 0.45, 0.94);
}
</style>

<style>
/* 逐字符淡入+上浮动画。keyframes 必须全局：span 由 JS 动态生成，scoped 选择器无法命中 */
@keyframes tw-char-rise {
  from {
    opacity: 0;
    transform: translateY(0.35em);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>

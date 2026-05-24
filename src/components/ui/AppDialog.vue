<template>
  <Teleport to="body">
    <!-- Backdrop -->
    <Transition name="modal">
      <div>
        <div
          class="blur-overlay"
          v-if="shouldShowOverlay"
          :style="{ opacity: overlayOpacity }"
        ></div>

        <div
          v-if="dialogStore.isOpen"
          class="fixed inset-0 z-10001 flex items-center justify-center p-4"
        >
          <!-- Dialog card -->
          <div
            class="relative z-10 w-full max-w-md rounded-xl border border-white/20 bg-slate-900 p-8 overflow-hidden"
            @click.stop
          >
            <div
              class="absolute top-4 right-4 w-10 h-10 rounded-full flex items-center justify-center text-brand shadow-md transform -rotate-18"
            >
              <PawPrint :size="30" />
            </div>
            <div class="absolute top-0 left-0 right-0 h-1 bg-brand"></div>
            <!-- Title -->
            <div class="flex items-center gap-3 mb-4">
              <TriangleAlert
                v-if="dialogStore.type === 'alert'"
                class="w-6 h-6 text-brand shrink-0"
              />
              <Info v-else class="w-6 h-6 text-brand shrink-0" />
              <h3 class="text-xl font-bold text-white">{{ dialogStore.title }}</h3>
            </div>

            <!-- Message -->
            <p class="text-white/80 text-base leading-relaxed whitespace-pre-wrap">
              {{ dialogStore.message }}
            </p>

            <!-- Buttons -->
            <div class="flex justify-end gap-2.5 mt-6 pt-4 border-t border-slate-800/80">
              <button
                v-if="dialogStore.type === 'confirm'"
                @click="dialogStore.cancel()"
                ref="cancelBtn"
                class="px-6 py-1.5 rounded-xl bg-transparent text-white/70 hover:text-white hover:bg-white/10 border border-white/20 transition-all font-bold text-sm"
              >
                取消
              </button>
              <button
                @click="dialogStore.ok()"
                ref="okBtn"
                class="px-6 py-1.5 rounded-xl bg-cyan-500 hover:bg-cyan-400 text-white font-bold shadow-lg shadow-cyan-500/25 active:scale-100 transition-all text-sm"
              >
                确定
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { useDialogStore } from '../../stores/modules/ui/dialog'
import { Info, TriangleAlert, PawPrint } from 'lucide-vue-next'

const dialogStore = useDialogStore()
const okBtn = ref<HTMLElement | null>(null)
const cancelBtn = ref<HTMLElement | null>(null)

// 添加延迟状态
const shouldShowOverlay = ref(false)
const overlayOpacity = ref(0)

watch(
  () => dialogStore.isOpen,
  (open) => {
    if (open) {
      nextTick(() => okBtn.value?.focus())
    }
  },
)

watch(
  () => dialogStore.isOpen,
  (newVal) => {
    if (newVal) {
      // 显示时：立即显示元素，然后延迟改变透明度
      shouldShowOverlay.value = true
      setTimeout(() => {
        overlayOpacity.value = 1
      }, 10) // 使用很小的延迟确保浏览器有机会渲染
    } else {
      // 隐藏时：先改变透明度，然后延迟隐藏元素
      overlayOpacity.value = 0
      setTimeout(() => {
        shouldShowOverlay.value = false
      }, 100) // 匹配你的动画持续时间
    }
  },
  { immediate: true },
)

const handleKeydown = (e: KeyboardEvent) => {
  if (!dialogStore.isOpen) return
  if (e.key === 'Escape') {
    e.preventDefault()
    if (dialogStore.type === 'alert') {
      dialogStore.ok()
    } else {
      dialogStore.cancel()
    }
  } else if (e.key === 'Enter') {
    e.preventDefault()
    dialogStore.ok()
  }
}

onMounted(() => window.addEventListener('keydown', handleKeydown))
onUnmounted(() => window.removeEventListener('keydown', handleKeydown))
</script>

<style scoped>
.modal-enter-active,
.modal-leave-active {
  transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
  transform: scale(1) translateY(10px);
}

.blur-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 10000;

  /* 初始状态 */
  opacity: 0;
  backdrop-filter: blur(5px);
  background-color: rgba(0, 0, 0, 0.45);

  /* 过渡效果 */
  transition: opacity 0.3s ease;

  /* 确保覆盖层可以点击穿透 */
  pointer-events: none;
}
</style>

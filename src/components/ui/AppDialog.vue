<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="dialogStore.isOpen"
        class="fixed inset-0 z-[10001] flex items-center justify-center p-4"
      >
        <!-- Backdrop -->
        <div
          class="absolute inset-0 bg-slate-900/60 backdrop-blur-sm"
          @click="dialogStore.type === 'alert' ? dialogStore.ok() : dialogStore.cancel()"
        ></div>

        <!-- Dialog card -->
        <div
          class="relative z-10 w-full max-w-md rounded-3xl border border-white/20 bg-slate-900/40 backdrop-blur-2xl shadow-2xl p-8"
          @click.stop
        >
          <!-- Title -->
          <div class="flex items-center gap-3 mb-4">
            <svg
              v-if="dialogStore.type === 'alert'"
              class="w-6 h-6 text-cyan-400 shrink-0"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
            <svg
              v-else
              class="w-6 h-6 text-amber-400 shrink-0"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
            <h3 class="text-xl font-bold text-white">{{ dialogStore.title }}</h3>
          </div>

          <!-- Message -->
          <p class="text-white/80 text-sm leading-relaxed whitespace-pre-wrap">
            {{ dialogStore.message }}
          </p>

          <!-- Buttons -->
          <div class="mt-8 flex gap-3 justify-end">
            <button
              v-if="dialogStore.type === 'confirm'"
              @click="dialogStore.cancel()"
              ref="cancelBtn"
              class="px-6 py-3 rounded-xl bg-transparent text-white/70 hover:text-white hover:bg-white/10 border border-white/20 transition-all font-bold text-sm"
            >
              取消
            </button>
            <button
              @click="dialogStore.ok()"
              ref="okBtn"
              class="px-6 py-3 rounded-xl bg-cyan-500 hover:bg-cyan-400 text-white font-bold shadow-lg shadow-cyan-500/25 active:scale-95 transition-all text-sm"
            >
              确定
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { useDialogStore } from '../../stores/modules/ui/dialog'

const dialogStore = useDialogStore()
const okBtn = ref<HTMLElement | null>(null)
const cancelBtn = ref<HTMLElement | null>(null)

watch(
  () => dialogStore.isOpen,
  (open) => {
    if (open) {
      nextTick(() => okBtn.value?.focus())
    }
  },
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
  transform: scale(0.95) translateY(10px);
}
</style>

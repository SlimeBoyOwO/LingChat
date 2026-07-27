<template>
  <Transition name="pet-notify">
    <div
      v-if="uiStore.notification.isVisible"
      class="w-full flex justify-center z-30"
    >
      <div
        class="flex flex-col gap-0.5
               w-[calc(90%*var(--pet-ui-scale,1))] max-w-[calc(220px*var(--pet-ui-scale,1))]
               p-[calc(6px*var(--pet-ui-scale,1))] px-[calc(10px*var(--pet-ui-scale,1))]
               rounded-[calc(10px*var(--pet-ui-scale,1))]
               bg-neutral-950/75 backdrop-blur-xl backdrop-saturate-200
               border border-white/10 border-l-4
               shadow-[0_4px_16px_rgba(0,0,0,0.4),inset_0_1px_0_rgba(255,255,255,0.06)]
               [text-shadow:0_1px_3px_rgba(0,0,0,0.5)]"
        :class="[typeBorderClass, typeTitleClass]"
      >
        <div class="truncate
                    text-[calc(12px*var(--pet-ui-scale,1))] font-semibold leading-snug
                    text-white/95">
          {{ uiStore.notification.title || '通知' }}
        </div>
        <div class="line-clamp-2
                    text-[calc(11px*var(--pet-ui-scale,1))] leading-snug
                    text-white/75">
          {{ uiStore.notification.message }}
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUIStore } from '../../stores/modules/ui/ui'
import type { NotificationType } from '../../stores/modules/ui/ui'

const uiStore = useUIStore()

const typeBorderClass = computed(() => {
  const map: Record<NotificationType, string> = {
    error:   'border-l-red-400/80',
    success: 'border-l-green-400/80',
    info:    'border-l-cyan-400/80',
    warning: 'border-l-amber-400/80',
  }
  return map[uiStore.notification.type] || map.info
})

const typeTitleClass = computed(() => {
  const map: Record<NotificationType, string> = {
    error:   'text-red-300/95',
    success: 'text-green-300/95',
    info:    'text-cyan-300/95',
    warning: 'text-amber-200/95',
  }
  return map[uiStore.notification.type] || map.info
})
</script>

<style scoped>
@reference "tailwindcss";

/* ── 进入/离开动画（Vue Transition 需要自定义 CSS）── */
.pet-notify-enter-active,
.pet-notify-leave-active {
  transition:
    opacity 0.25s cubic-bezier(0.16, 1, 0.3, 1),
    transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.pet-notify-enter-from {
  opacity: 0;
  transform: translateY(-6px) scale(0.95);
}

.pet-notify-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.95);
}
</style>


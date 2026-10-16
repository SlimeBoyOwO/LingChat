<template>
  <Transition name="notify-fade">
    <div
      v-if="uiStore.notification.isVisible"
      class="fixed top-[calc(20px+var(--safe-area-inset-top))] left-[calc(16px+var(--safe-area-inset-left))] z-[10000] flex max-w-[320px] min-w-[220px] flex-col gap-0.5 rounded-xl border border-l-4 border-white/10 bg-neutral-950/75 p-2.5 px-3.5 shadow-[0_4px_16px_rgba(0,0,0,0.4),inset_0_1px_0_rgba(255,255,255,0.06)] backdrop-blur-xl backdrop-saturate-200 [text-shadow:0_1px_3px_rgba(0,0,0,0.5)]"
      :class="typeBorderClass"
    >
      <div class="truncate text-sm leading-snug font-semibold" :class="typeTitleClass">
        {{ uiStore.notification.title || $t("ui.notification.titlePlaceholder") }}
      </div>
      <div
        v-if="uiStore.notification.message"
        class="line-clamp-2 text-xs leading-snug text-white/75"
      >
        {{ uiStore.notification.message }}
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useUIStore } from "../../stores/modules/ui/ui";
import type { NotificationType } from "../../stores/modules/ui/ui";

const uiStore = useUIStore();

const typeBorderClass = computed(() => {
  const map: Record<NotificationType, string> = {
    error: "border-l-red-400/80",
    success: "border-l-green-400/80",
    info: "border-l-cyan-400/80",
    warning: "border-l-amber-400/80",
  };
  return map[uiStore.notification.type] || map.info;
});

const typeTitleClass = computed(() => {
  const map: Record<NotificationType, string> = {
    error: "text-red-300/95",
    success: "text-green-300/95",
    info: "text-cyan-300/95",
    warning: "text-amber-200/95",
  };
  return map[uiStore.notification.type] || map.info;
});
</script>

<style scoped>
.notify-fade-enter-active,
.notify-fade-leave-active {
  transition:
    opacity 0.25s cubic-bezier(0.16, 1, 0.3, 1),
    transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.notify-fade-enter-from,
.notify-fade-leave-to {
  opacity: 0;
  transform: translateX(-8px) scale(0.96);
}
</style>

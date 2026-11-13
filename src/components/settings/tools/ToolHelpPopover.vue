<template>
  <span class="inline-flex">
    <button
      ref="triggerRef"
      type="button"
      class="flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-full border border-white/25 text-[11px] leading-none text-white/60 transition-colors duration-150 hover:border-white/50 hover:text-white"
      :aria-label="$t('ui.toolCalls.expandHelp')"
      @click.stop="toggle"
    >
      ?
    </button>

    <Teleport to="body">
      <Transition name="tool-help-pop">
        <div
          v-if="open"
          ref="panelRef"
          class="fixed z-9999 max-w-[calc(100vw-24px)] rounded-xl border border-white/15 bg-[#1b1b24] px-3.5 py-3 text-xs leading-relaxed text-white/80 shadow-[0_8px_32px_rgba(0,0,0,0.6)]"
          :style="panelStyle"
          role="tooltip"
          @click.stop
        >
          {{ text }}
        </div>
      </Transition>
    </Teleport>
  </span>
</template>

<script setup lang="ts">
import { onUnmounted, ref, watch } from "vue";

defineProps<{ text: string }>();

const open = ref(false);
const triggerRef = ref<HTMLButtonElement>();
const panelRef = ref<HTMLElement>();
const panelStyle = ref<Record<string, string>>({});

function reposition() {
  const el = triggerRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const width = Math.min(280, window.innerWidth - 24);
  const left = Math.max(12, Math.min(rect.left, window.innerWidth - width - 12));
  const gap = 6;
  const spaceBelow = window.innerHeight - rect.bottom - gap - 12;
  const openUp = spaceBelow < 120 && rect.top > spaceBelow;
  panelStyle.value = {
    width: `${width}px`,
    left: `${left}px`,
    top: openUp ? "" : `${rect.bottom + gap}px`,
    bottom: openUp ? `${window.innerHeight - rect.top + gap}px` : "",
  };
}

function onDocMouseDown(event: MouseEvent) {
  const target = event.target as Node;
  if (triggerRef.value?.contains(target) || panelRef.value?.contains(target)) return;
  open.value = false;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") open.value = false;
}

function toggle() {
  open.value = !open.value;
}

watch(open, (value) => {
  if (value) {
    reposition();
    document.addEventListener("mousedown", onDocMouseDown);
    document.addEventListener("keydown", onKeydown);
    window.addEventListener("scroll", reposition, true);
    window.addEventListener("resize", reposition);
  } else {
    document.removeEventListener("mousedown", onDocMouseDown);
    document.removeEventListener("keydown", onKeydown);
    window.removeEventListener("scroll", reposition, true);
    window.removeEventListener("resize", reposition);
  }
});

onUnmounted(() => {
  document.removeEventListener("mousedown", onDocMouseDown);
  document.removeEventListener("keydown", onKeydown);
  window.removeEventListener("scroll", reposition, true);
  window.removeEventListener("resize", reposition);
});
</script>

<style scoped>
.tool-help-pop-enter-active,
.tool-help-pop-leave-active {
  transition:
    opacity 0.16s ease,
    transform 0.16s ease;
}
.tool-help-pop-enter-from,
.tool-help-pop-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.98);
}
</style>

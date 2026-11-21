<template>
  <section
    role="dialog"
    aria-modal="true"
    :aria-labelledby="titleId"
    class="glass-effect flex h-[85dvh] w-full max-w-4xl flex-col overflow-hidden rounded-3xl text-white"
    @click.stop
  >
    <header
      class="flex shrink-0 items-center justify-between gap-4 border-b border-white/10 bg-white/5 p-4 sm:p-6"
    >
      <div class="flex min-w-0 items-center gap-4">
        <div
          v-if="$slots.icon"
          class="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-white/10 shadow-inner"
        >
          <slot name="icon" />
        </div>
        <div class="min-w-0">
          <h2 :id="titleId" class="m-0 text-xl font-bold drop-shadow-[0_2px_4px_rgba(0,0,0,0.3)]">
            {{ title }}
          </h2>
          <p v-if="subtitle" class="m-0 text-sm text-white/50">{{ subtitle }}</p>
        </div>
      </div>
      <button
        type="button"
        :aria-label="$t('common.close')"
        :disabled="busy"
        class="flex h-9 w-9 shrink-0 cursor-pointer items-center justify-center rounded-full border-none bg-white/10 text-white transition-all duration-200 enabled:hover:rotate-90 enabled:hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-40"
        @click="close"
      >
        <X :size="24" />
      </button>
    </header>
    <div class="flex min-h-0 flex-1 flex-col overflow-hidden sm:flex-row">
      <slot name="sidebar" />
      <div class="relative min-h-0 min-w-0 flex-1 overflow-y-auto p-4 sm:p-6"><slot /></div>
    </div>
    <footer v-if="$slots.footer" class="shrink-0 border-t border-white/10 bg-white/5 p-4">
      <slot name="footer" />
    </footer>
    <slot name="overlay" />
  </section>
</template>
<script setup lang="ts">
import { useId } from "vue";
import { X } from "@lucide/vue";
const props = defineProps<{ title: string; subtitle?: string; busy?: boolean }>();
const emit = defineEmits<{ close: [] }>();
const titleId = useId();
function close() {
  if (!props.busy) emit("close");
}
</script>

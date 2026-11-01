<template>
  <div class="rounded-xl border px-4 py-3" :class="toneClass">
    <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div class="min-w-0">
        <h3 class="font-semibold text-white">{{ title }}</h3>
        <p v-if="description" class="mt-1 text-sm leading-relaxed text-gray-300">
          {{ description }}
        </p>
      </div>
      <div v-if="$slots.action" class="shrink-0">
        <slot name="action" />
      </div>
    </div>
    <slot />
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    title: string;
    description?: string;
    tone?: "default" | "warning";
  }>(),
  { tone: "default" },
);

const toneClass = computed(() =>
  props.tone === "warning" ? "border-amber-400/30 bg-amber-400/5" : "border-white/15 bg-white/5",
);
</script>

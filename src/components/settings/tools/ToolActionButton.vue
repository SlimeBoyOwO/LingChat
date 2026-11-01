<template>
  <button
    type="button"
    class="rounded-lg border px-4 py-2 text-sm transition-colors duration-200 disabled:cursor-wait disabled:opacity-60"
    :class="
      variant === 'warning'
        ? 'border-amber-400/60 bg-amber-400/15 font-semibold text-amber-200 hover:bg-amber-400/25'
        : 'border-white/20 bg-white/10 font-medium text-white hover:bg-white/20'
    "
    v-bind="$attrs"
  >
    <slot />
  </button>
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false });

withDefaults(defineProps<{ variant?: "default" | "warning" }>(), { variant: "default" });
</script>

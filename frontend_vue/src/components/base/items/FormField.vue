<template>
  <div :class="['flex flex-col gap-1', $attrs.class || '']">
    <label class="text-white/70 text-xs font-medium">{{ label }}</label>
    <slot />
  </div>
</template>

<script setup lang="ts">
defineProps<{ label: string }>()
</script>

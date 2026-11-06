<template>
  <label class="block">
    <span class="text-brand text-sm font-medium">{{ label }}</span>
    <slot />
    <span v-if="hint" class="mt-1 block text-xs leading-relaxed text-gray-400">{{ hint }}</span>
  </label>
</template>

<script setup lang="ts">
defineProps<{
  label: string;
  hint?: string;
}>();
</script>

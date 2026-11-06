<template>
  <select
    v-model="model"
    class="select-dark focus:border-brand focus:ring-brand/20 mt-2 w-full cursor-pointer rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white transition-colors duration-200 focus:ring-2 focus:outline-none"
    v-bind="$attrs"
  >
    <slot />
  </select>
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false });

const model = defineModel<string>({ required: true });
</script>

<template>
  <textarea
    v-model="model"
    :rows="rows"
    class="focus:border-brand focus:ring-brand/20 mt-2 w-full resize-y rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white transition-colors duration-200 focus:ring-2 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
    v-bind="$attrs"
  ></textarea>
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false });

withDefaults(defineProps<{ rows?: number }>(), { rows: 3 });
const model = defineModel<string>({ required: true });
</script>

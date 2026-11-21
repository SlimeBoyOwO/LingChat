<template>
  <nav
    :aria-label="label"
    class="tab-sidebar-scroll flex w-full shrink-0 flex-row gap-2 overflow-x-auto border-b border-white/10 bg-black/10 p-2 sm:w-44 sm:flex-col sm:overflow-x-hidden sm:overflow-y-auto sm:border-r sm:border-b-0 sm:p-3"
  >
    <button
      v-for="tab in tabs"
      :key="tab.id"
      type="button"
      :aria-pressed="modelValue === tab.id"
      :disabled="disabled"
      class="shrink-0 cursor-pointer rounded-xl border-none bg-transparent px-4 py-2.5 text-left font-medium text-white/60 transition-all duration-200 enabled:hover:bg-white/5 enabled:hover:text-white disabled:cursor-not-allowed disabled:opacity-40 sm:w-full"
      :class="{ 'bg-[rgba(94,114,228,0.2)] font-semibold! text-[#79d9ff]!': modelValue === tab.id }"
      @click="emit('update:modelValue', tab.id)"
    >
      {{ tab.label }}
    </button>
  </nav>
</template>
<script setup lang="ts" generic="T extends string">
defineProps<{
  modelValue: T;
  tabs: { id: T; label: string }[];
  label: string;
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: T] }>();
</script>
<style scoped>
.tab-sidebar-scroll {
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.18) transparent;
}
.tab-sidebar-scroll::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
.tab-sidebar-scroll::-webkit-scrollbar-track {
  background: transparent;
}
.tab-sidebar-scroll::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.18);
  border-radius: 3px;
}
.tab-sidebar-scroll::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.32);
}
</style>

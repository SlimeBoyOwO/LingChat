<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="flex shrink-0 items-center gap-2 border-b border-white/10 px-1 pb-3">
      <button
        type="button"
        class="flex h-8 w-8 items-center justify-center rounded-lg border border-white/15 bg-white/5 text-lg leading-none text-white/80 transition-colors duration-150 hover:bg-white/10 hover:text-white"
        :aria-label="$t('ui.toolCalls.back')"
        @click="$emit('back')"
      >
        ‹
      </button>
      <h2 class="text-brand truncate text-lg font-semibold">{{ title }}</h2>
    </div>

    <div class="custom-scrollbar min-h-0 flex-1 overflow-y-auto px-1 py-3">
      <slot />
    </div>

    <div
      class="flex shrink-0 items-center gap-2 border-t border-white/10 px-1 pt-3"
      :class="showReset ? 'justify-between' : 'justify-end'"
    >
      <button
        v-if="showReset"
        type="button"
        class="rounded-lg border border-white/15 bg-white/5 px-4 py-2 text-sm text-gray-300 transition-colors duration-150 hover:bg-white/10 hover:text-white"
        @click="$emit('reset')"
      >
        {{ $t("ui.toolCalls.resetPackage") }}
      </button>
      <button
        type="button"
        class="bg-brand rounded-lg px-5 py-2 text-sm font-medium text-white transition-colors duration-150 hover:bg-[#0056b3]"
        @click="$emit('save')"
      >
        {{ $t("ui.toolCalls.save") }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  title: string;
  showReset?: boolean;
}>();

defineEmits<{
  back: [];
  save: [];
  reset: [];
}>();
</script>

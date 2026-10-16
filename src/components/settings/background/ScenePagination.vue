<template>
  <div v-if="totalPages > 1" class="flex items-center justify-center gap-2 pb-2">
    <button
      :disabled="modelValue <= 1"
      @click="$emit('update:modelValue', 1)"
      class="px-2 py-1 text-xs text-white/50 transition-colors hover:text-white disabled:cursor-not-allowed disabled:opacity-30"
    >
      {{ $t("settings.background.pagination.first") }}
    </button>
    <button
      :disabled="modelValue <= 1"
      @click="$emit('update:modelValue', modelValue - 1)"
      class="px-3 py-1 text-sm text-white/50 transition-colors hover:text-white disabled:cursor-not-allowed disabled:opacity-30"
    >
      {{ $t("settings.shared.prevPage") }}
    </button>
    <span class="px-3 text-xs text-white/60">
      {{ $t("settings.shared.pageOf", { current: modelValue, total: totalPages }) }}
    </span>
    <button
      :disabled="modelValue >= totalPages"
      @click="$emit('update:modelValue', modelValue + 1)"
      class="px-3 py-1 text-sm text-white/50 transition-colors hover:text-white disabled:cursor-not-allowed disabled:opacity-30"
    >
      {{ $t("settings.shared.nextPage") }}
    </button>
    <button
      :disabled="modelValue >= totalPages"
      @click="$emit('update:modelValue', totalPages)"
      class="px-2 py-1 text-xs text-white/50 transition-colors hover:text-white disabled:cursor-not-allowed disabled:opacity-30"
    >
      {{ $t("settings.background.pagination.last") }}
    </button>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  /** 当前页码，从 1 开始 */
  modelValue: number;
  totalPages: number;
}>();

defineEmits<{
  (e: "update:modelValue", value: number): void;
}>();
</script>

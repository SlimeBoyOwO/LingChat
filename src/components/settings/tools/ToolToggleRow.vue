<template>
  <div :class="rowClass">
    <Toggle class="w-full" :checked="checked" :disabled="disabled" @change="onChange">
      <span class="ml-1 text-sm text-gray-300">{{ label }}</span>
    </Toggle>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import Toggle from "@/components/base/widget/Toggle.vue";

const props = withDefaults(
  defineProps<{
    checked: boolean;
    label: string;
    disabled?: boolean;
    variant?: "row" | "card" | "bare";
  }>(),
  { disabled: false, variant: "row" },
);
const emit = defineEmits<{ change: [value: boolean] }>();

const rowClass = computed(() => {
  if (props.variant === "card") return "flex items-center rounded-lg bg-white/5 px-3 py-2.5";
  if (props.variant === "bare") return "flex items-center";
  return "flex items-center px-1 py-2.5";
});

function onChange(value: boolean) {
  emit("change", value);
}
</script>

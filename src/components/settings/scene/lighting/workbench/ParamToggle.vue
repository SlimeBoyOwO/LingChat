<script setup lang="ts">
/** 开关参数行：标签 + 紫色开关。 */
defineProps<{
  modelValue: boolean;
  label: string;
  hint?: string;
}>();

const emit = defineEmits<{ (e: "update:modelValue", v: boolean): void }>();
</script>

<template>
  <button
    type="button"
    class="wb-param"
    style="display: flex; align-items: center; width: 100%; text-align: left"
    :title="hint || label"
    @click="emit('update:modelValue', !modelValue)"
  >
    <span class="wb-param-label" style="color: var(--wb-text-2)">{{ label }}</span>
    <span style="margin-left: auto; display: flex">
      <span class="wb-switch" :class="{ 'is-on': modelValue }" />
    </span>
  </button>
</template>

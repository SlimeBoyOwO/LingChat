<template>
  <ImageGenerationConfig
    :model-value="modelValue.image_generation"
    @update:model-value="$emit('update:modelValue', { ...modelValue, image_generation: $event })"
  />
</template>
<script setup lang="ts">
import type { ToolSettings } from "@/api/services/tool-settings";
import ImageGenerationConfig from "@/components/settings/background/ImageGenerationConfig.vue";
defineProps<{ modelValue: ToolSettings }>();
defineEmits<{ "update:modelValue": [value: ToolSettings] }>();
</script>

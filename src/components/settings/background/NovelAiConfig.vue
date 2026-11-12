<template>
  <div class="grid gap-4 sm:grid-cols-2">
    <template v-if="!advanced">
      <ToolField :label="t('imageGeneration.novelaiToken')" class="sm:col-span-2">
        <ToolInput
          type="password"
          autocomplete="off"
          :model-value="modelValue.api_key"
          :disabled="disabled"
          @update:model-value="update('api_key', String($event))"
        />
      </ToolField>
      <ToolField :label="t('imageGeneration.baseUrl')" class="sm:col-span-2">
        <ToolInput
          :model-value="modelValue.base_url"
          :placeholder="t('imageGeneration.novelaiDefaultUrl')"
          :disabled="disabled"
          @update:model-value="update('base_url', String($event))"
        />
      </ToolField>
      <ToolField
        v-if="!hideNegativeTags"
        :label="t('imageGeneration.negativeTags')"
        :hint="t('imageGeneration.negativeTagsHint')"
        class="sm:col-span-2"
      >
        <ToolTextarea
          :rows="3"
          :model-value="modelValue.negative_prompt"
          :disabled="disabled"
          :placeholder="t('imageGeneration.negativeTagsPlaceholder')"
          @update:model-value="update('negative_prompt', $event)"
        />
      </ToolField>
    </template>
    <template v-else>
      <ToolField
        v-for="field in numericFields"
        :key="field.key"
        :label="t('imageGeneration.' + field.label)"
      >
        <ToolInput
          type="number"
          :model-value="modelValue[field.key]"
          :disabled="disabled"
          :min="field.min"
          :max="field.max"
          :step="field.step"
          @update:model-value="update(field.key, Number($event))"
        />
      </ToolField>
      <ToolField :label="t('imageGeneration.sampler')">
        <ToolInput
          :model-value="modelValue.sampler"
          :disabled="disabled"
          @update:model-value="update('sampler', String($event))"
        />
      </ToolField>
      <ToolField :label="t('imageGeneration.seed')">
        <ToolInput
          type="number"
          :model-value="modelValue.seed ?? ''"
          :disabled="disabled"
          min="0"
          max="4294967295"
          @update:model-value="update('seed', String($event).trim() ? Number($event) : null)"
        />
      </ToolField>
      <ToolField :label="t('imageGeneration.transport')">
        <ToolSelect
          :model-value="modelValue.transport"
          :disabled="disabled"
          @update:model-value="update('transport', $event as NovelAiSettings['transport'])"
        >
          <option value="auto">{{ t("imageGeneration.transportAuto") }}</option>
          <option value="json">JSON / ZIP</option>
          <option value="sse">SSE</option>
        </ToolSelect>
      </ToolField>
      <ToolField :label="t('imageGeneration.frontendUrl')">
        <ToolInput
          :model-value="modelValue.frontend_url"
          :disabled="disabled"
          :placeholder="t('imageGeneration.novelaiDefaultUrl')"
          @update:model-value="update('frontend_url', String($event))"
        />
      </ToolField>
      <ToolField :label="t('imageGeneration.parametersJson')" class="sm:col-span-2">
        <ToolTextarea
          :rows="4"
          spellcheck="false"
          class="font-mono"
          :model-value="modelValue.parameters_json"
          :disabled="disabled"
          @update:model-value="update('parameters_json', $event)"
        />
      </ToolField>
    </template>
  </div>
</template>
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { NovelAiSettings } from "@/api/services/image-generation";
import ToolField from "@/components/settings/tools/fields/ToolField.vue";
import ToolInput from "@/components/settings/tools/fields/ToolInput.vue";
import ToolSelect from "@/components/settings/tools/fields/ToolSelect.vue";
import ToolTextarea from "@/components/settings/tools/fields/ToolTextarea.vue";
const props = defineProps<{
  modelValue: NovelAiSettings;
  advanced?: boolean;
  disabled?: boolean;
  hideNegativeTags?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: NovelAiSettings] }>();
const { t } = useI18n();
const numericFields = [
  { key: "width", label: "width", min: 64, max: 4096, step: 64 },
  { key: "height", label: "height", min: 64, max: 4096, step: 64 },
  { key: "steps", label: "steps", min: 1, max: 100, step: 1 },
  { key: "scale", label: "scale", min: 0.1, max: 100, step: 0.1 },
  { key: "strength", label: "strength", min: 0, max: 1, step: 0.05 },
  { key: "noise", label: "noise", min: 0, max: 1, step: 0.05 },
] as const;
function update<K extends keyof NovelAiSettings>(key: K, value: NovelAiSettings[K]) {
  emit("update:modelValue", { ...props.modelValue, [key]: value });
}
</script>

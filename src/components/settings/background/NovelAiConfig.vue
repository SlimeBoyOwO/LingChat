<template>
  <div class="grid gap-3 sm:grid-cols-2">
    <template v-if="!advanced">
      <label class="flex flex-col gap-1.5 sm:col-span-2">
        {{ t("imageGeneration.novelaiToken") }}
        <input
          type="password"
          autocomplete="off"
          :value="modelValue.api_key"
          :disabled="disabled"
          class="rounded-lg border border-white/20 bg-black/25 px-3 py-2"
          @change="update('api_key', value($event))"
        />
      </label>
      <label class="flex flex-col gap-1.5 sm:col-span-2">
        {{ t("imageGeneration.baseUrl") }}
        <input
          :value="modelValue.base_url"
          :placeholder="t('imageGeneration.novelaiDefaultUrl')"
          :disabled="disabled"
          class="rounded-lg border border-white/20 bg-black/25 px-3 py-2"
          @change="update('base_url', value($event))"
        />
      </label>
    </template>
    <template v-else>
      <label v-for="field in numericFields" :key="field.key" class="flex flex-col gap-1.5">
        {{ t("imageGeneration." + field.label) }}
        <input
          type="number"
          :value="modelValue[field.key]"
          :disabled="disabled"
          :min="field.min"
          :max="field.max"
          :step="field.step"
          class="rounded-lg border border-white/20 bg-black/25 px-3 py-2"
          @change="update(field.key, Number(value($event)))"
        />
      </label>
      <label class="flex flex-col gap-1.5">
        {{ t("imageGeneration.sampler") }}
        <input
          :value="modelValue.sampler"
          :disabled="disabled"
          class="rounded-lg border border-white/20 bg-black/25 px-3 py-2"
          @change="update('sampler', value($event))"
        />
      </label>
      <label class="flex flex-col gap-1.5">
        {{ t("imageGeneration.seed") }}
        <input
          type="number"
          :value="modelValue.seed ?? ''"
          :disabled="disabled"
          min="0"
          max="4294967295"
          class="rounded-lg border border-white/20 bg-black/25 px-3 py-2"
          @change="update('seed', value($event).trim() ? Number(value($event)) : null)"
        />
      </label>
      <label class="flex flex-col gap-1.5 sm:col-span-2">
        {{ t("imageGeneration.negativePrompt") }}
        <textarea
          rows="2"
          :value="modelValue.negative_prompt"
          :disabled="disabled"
          class="rounded-lg border border-white/20 bg-black/25 px-3 py-2"
          @change="update('negative_prompt', value($event))"
        />
      </label>
      <label class="flex flex-col gap-1.5">
        {{ t("imageGeneration.transport") }}
        <select
          :value="modelValue.transport"
          :disabled="disabled"
          class="rounded-lg border border-white/20 bg-slate-900 px-3 py-2"
          @change="update('transport', value($event) as NovelAiSettings['transport'])"
        >
          <option value="auto">{{ t("imageGeneration.transportAuto") }}</option>
          <option value="json">JSON / ZIP</option>
          <option value="sse">SSE</option>
        </select>
      </label>
      <label class="flex flex-col gap-1.5">
        {{ t("imageGeneration.frontendUrl") }}
        <input
          :value="modelValue.frontend_url"
          :disabled="disabled"
          :placeholder="t('imageGeneration.novelaiDefaultUrl')"
          class="rounded-lg border border-white/20 bg-black/25 px-3 py-2"
          @change="update('frontend_url', value($event))"
        />
      </label>
      <label class="flex flex-col gap-1.5 sm:col-span-2">
        {{ t("imageGeneration.parametersJson") }}
        <textarea
          rows="4"
          spellcheck="false"
          :value="modelValue.parameters_json"
          :disabled="disabled"
          class="rounded-lg border border-white/20 bg-black/25 px-3 py-2 font-mono"
          @change="update('parameters_json', value($event))"
        />
      </label>
    </template>
  </div>
</template>
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { NovelAiSettings } from "@/api/services/image-generation";
const props = defineProps<{
  modelValue: NovelAiSettings;
  advanced?: boolean;
  disabled?: boolean;
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
function value(event: Event) {
  return (event.target as HTMLInputElement).value;
}
function update<K extends keyof NovelAiSettings>(key: K, value: NovelAiSettings[K]) {
  emit("update:modelValue", { ...props.modelValue, [key]: value });
}
</script>

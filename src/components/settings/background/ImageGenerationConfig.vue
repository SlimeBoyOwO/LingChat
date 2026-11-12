<template>
  <div class="flex flex-col gap-4 text-sm text-white/85">
    <label class="flex flex-col gap-1.5">
      {{ t("imageGeneration.provider") }}
      <select
        :value="modelValue.provider"
        class="rounded-lg border border-white/20 bg-slate-900 px-3 py-2"
        :disabled="disabled"
        @change="changeProvider(($event.target as HTMLSelectElement).value)"
      >
        <option v-for="provider in providers" :key="provider.id" :value="provider.id">
          {{ provider.label }}
        </option>
      </select>
    </label>
    <template v-if="custom">
      <label class="flex flex-col gap-1.5">
        {{ t("imageGeneration.baseUrl") }}
        <input
          :value="modelValue.base_url"
          class="rounded-lg border border-white/20 bg-black/25 px-3 py-2"
          :disabled="disabled"
          @change="update('base_url', ($event.target as HTMLInputElement).value)"
        />
      </label>
      <label class="flex flex-col gap-1.5">
        {{ t("imageGeneration.apiKey") }}
        <input
          type="password"
          autocomplete="off"
          :value="modelValue.api_key"
          class="rounded-lg border border-white/20 bg-black/25 px-3 py-2"
          :disabled="disabled"
          @change="update('api_key', ($event.target as HTMLInputElement).value)"
        />
      </label>
    </template>
    <NovelAiConfig
      v-if="isNovelAi && modelValue.novelai"
      :model-value="modelValue.novelai"
      :disabled="disabled"
      @update:model-value="update('novelai', $event)"
    />
    <label class="flex flex-col gap-1.5">
      {{ t("imageGeneration.model") }}
      <div class="flex gap-2">
        <input
          :value="modelValue.model"
          :list="modelListId"
          :placeholder="modelPlaceholder"
          class="min-w-0 flex-1 rounded-lg border border-white/20 bg-black/25 px-3 py-2"
          :disabled="disabled"
          @input="update('model', ($event.target as HTMLInputElement).value)"
        />
        <datalist :id="modelListId">
          <option v-for="model in catalog?.models ?? []" :key="model" :value="model" />
        </datalist>
        <button
          type="button"
          class="rounded-lg border border-cyan-300/30 bg-cyan-300/10 px-3 py-2 text-cyan-100 disabled:opacity-40"
          :disabled="loading || disabled"
          @click="refreshModels(true)"
        >
          {{ loading ? t("imageGeneration.loading") : t("imageGeneration.refreshModels") }}
        </button>
      </div>
      <select
        v-if="catalog?.models.length"
        class="rounded-lg border border-white/20 bg-slate-900 px-3 py-2"
        :value="modelValue.model"
        :disabled="disabled"
        @change="update('model', ($event.target as HTMLSelectElement).value)"
      >
        <option value="" :disabled="!catalog.allow_automatic">{{ modelPlaceholder }}</option>
        <option
          v-if="modelValue.model && !catalog.models.includes(modelValue.model)"
          :value="modelValue.model"
        >
          {{ modelValue.model }}
        </option>
        <option v-for="model in catalog.models" :key="model" :value="model">{{ model }}</option>
      </select>
    </label>
    <p class="text-xs text-white/55">{{ t("imageGeneration.modelHint") }}</p>
    <p v-if="catalog?.warning" class="text-xs text-amber-200">{{ catalog.warning }}</p>
    <p v-if="error" role="alert" class="text-sm text-red-300">{{ error }}</p>
    <details class="rounded-lg border border-white/10 p-3">
      <summary class="cursor-pointer text-white/65">{{ t("imageGeneration.advanced") }}</summary>
      <div class="mt-3 grid gap-3 sm:grid-cols-2">
        <label v-if="!isNovelAi" class="flex flex-col gap-1"
          >{{ t("imageGeneration.size") }}
          <input
            :value="modelValue.size"
            :disabled="disabled"
            class="rounded border border-white/20 bg-black/25 px-2 py-1.5"
            @change="update('size', ($event.target as HTMLInputElement).value)"
          />
        </label>
        <label v-if="!isNovelAi" class="flex flex-col gap-1"
          >{{ t("imageGeneration.quality") }}
          <input
            :value="modelValue.quality"
            :disabled="disabled"
            class="rounded border border-white/20 bg-black/25 px-2 py-1.5"
            @change="update('quality', ($event.target as HTMLInputElement).value)"
          />
        </label>
        <label class="flex flex-col gap-1"
          >{{ t("imageGeneration.timeout") }}
          <input
            type="number"
            min="30"
            max="900"
            :value="modelValue.timeout_secs"
            :disabled="disabled"
            class="rounded border border-white/20 bg-black/25 px-2 py-1.5"
            @change="update('timeout_secs', Number(($event.target as HTMLInputElement).value))"
          />
        </label>
        <label class="flex flex-col gap-1"
          >{{ t("imageGeneration.catalogUrl") }}
          <input
            :value="isNovelAi ? modelValue.novelai?.models_url : modelValue.models_url"
            :disabled="disabled"
            class="rounded border border-white/20 bg-black/25 px-2 py-1.5"
            @change="updateCatalogUrl(($event.target as HTMLInputElement).value)"
          />
        </label>
      </div>
      <NovelAiConfig
        v-if="isNovelAi && modelValue.novelai"
        :model-value="modelValue.novelai"
        :disabled="disabled"
        advanced
        @update:model-value="update('novelai', $event)"
        class="mt-4"
      />
      <p class="mt-2 text-xs text-white/50">{{ t("imageGeneration.advancedHint") }}</p>
    </details>
  </div>
</template>
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import NovelAiConfig from "./NovelAiConfig.vue";
import {
  listImageModels,
  listImageProviders,
  type ImageGenerationSettings,
  type ImageModelCatalog,
  type ImageProviderInfo,
} from "@/api/services/image-generation";
const props = defineProps<{ modelValue: ImageGenerationSettings; disabled?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [value: ImageGenerationSettings] }>();
const { t } = useI18n();
const providers = ref<ImageProviderInfo[]>([]);
const catalog = ref<ImageModelCatalog | null>(null);
const error = ref("");
const loading = ref(false);
const modelListId = useId();
const custom = computed(
  () => providers.value.find((p) => p.id === props.modelValue.provider)?.custom,
);
const isNovelAi = computed(
  () => providers.value.find((p) => p.id === props.modelValue.provider)?.protocol === "novelai",
);
const modelPlaceholder = computed(() =>
  error.value || catalog.value?.allow_automatic === false
    ? t("imageGeneration.manualModel")
    : t("imageGeneration.autoModel"),
);
let revision = 0;
function update<K extends keyof ImageGenerationSettings>(
  key: K,
  value: ImageGenerationSettings[K],
) {
  emit("update:modelValue", { ...props.modelValue, [key]: value });
}
function changeProvider(provider: string) {
  emit("update:modelValue", { ...props.modelValue, provider, model: "", models_url: "" });
}
function updateCatalogUrl(value: string) {
  if (isNovelAi.value && props.modelValue.novelai) {
    update("novelai", { ...props.modelValue.novelai, models_url: value });
  } else {
    update("models_url", value);
  }
}
async function refreshModels(force: boolean) {
  const request = ++revision;
  loading.value = true;
  error.value = "";
  try {
    const result = await listImageModels({ ...props.modelValue }, force);
    if (request === revision) catalog.value = result;
  } catch (cause) {
    if (request === revision) error.value = String(cause);
  } finally {
    if (request === revision) loading.value = false;
  }
}
watch(
  () => [
    props.modelValue.provider,
    props.modelValue.base_url,
    props.modelValue.api_key,
    props.modelValue.models_url,
    props.modelValue.novelai?.api_key,
    props.modelValue.novelai?.base_url,
    props.modelValue.novelai?.models_url,
    props.modelValue.novelai?.frontend_url,
  ],
  () => {
    revision++;
    catalog.value = null;
    error.value = "";
    loading.value = false;
    if (!custom.value || props.modelValue.base_url.trim()) void refreshModels(false);
  },
);
onMounted(async () => {
  try {
    providers.value = await listImageProviders();
    if (!custom.value || props.modelValue.base_url.trim()) await refreshModels(false);
  } catch (cause) {
    error.value = String(cause);
  }
});
onBeforeUnmount(() => {
  revision++;
});
</script>

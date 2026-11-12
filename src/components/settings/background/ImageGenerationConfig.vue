<template>
  <div class="space-y-4 text-sm">
    <ToolField v-if="!hideProvider" :label="t('imageGeneration.provider')">
      <ToolSelect
        :model-value="modelValue.provider"
        :disabled="disabled"
        :aria-label="t('imageGeneration.provider')"
        @update:model-value="changeProvider"
      >
        <option v-if="!selectedProvider" :value="modelValue.provider" disabled>
          {{ t("imageGeneration.providerUnavailable") }}
        </option>
        <option v-for="provider in providers" :key="provider.id" :value="provider.id">
          {{ provider.label }}
        </option>
      </ToolSelect>
    </ToolField>
    <template v-if="selectedProvider?.custom">
      <ToolField :label="t('imageGeneration.baseUrl')">
        <ToolInput
          :model-value="modelValue.base_url"
          :disabled="disabled"
          @update:model-value="update('base_url', String($event))"
        />
      </ToolField>
      <ToolField :label="t('imageGeneration.apiKey')">
        <ToolInput
          type="password"
          autocomplete="off"
          :model-value="modelValue.api_key"
          :disabled="disabled"
          @update:model-value="update('api_key', String($event))"
        />
      </ToolField>
    </template>
    <NovelAiConfig
      v-if="isNovelAi && modelValue.novelai"
      :model-value="modelValue.novelai"
      :disabled="disabled"
      :hide-negative-tags="hideNegativeTags"
      @update:model-value="update('novelai', $event)"
    />
    <ToolField :label="t('imageGeneration.model')" :hint="t('imageGeneration.modelHint')">
      <div class="flex flex-wrap items-start gap-2">
        <ToolInput
          v-if="manualModel"
          :model-value="modelValue.model"
          :placeholder="t('imageGeneration.manualModel')"
          :aria-label="t('imageGeneration.model')"
          autocomplete="off"
          class="min-w-0 flex-1"
          :disabled="disabled"
          @update:model-value="update('model', String($event))"
        />
        <ToolSelect
          v-else
          :model-value="modelValue.model"
          :aria-label="t('imageGeneration.model')"
          class="min-w-0 flex-1"
          :disabled="disabled"
          @update:model-value="update('model', $event)"
        >
          <option value="" :disabled="catalog?.allow_automatic === false">
            {{
              catalog?.allow_automatic === false
                ? t("imageGeneration.manualModel")
                : t("imageGeneration.autoModel")
            }}
          </option>
          <option
            v-if="modelValue.model && !catalog?.models.includes(modelValue.model)"
            :value="modelValue.model"
          >
            {{ modelValue.model }}
          </option>
          <option v-for="model in catalog?.models ?? []" :key="model" :value="model">
            {{ model }}
          </option>
        </ToolSelect>
        <ToolActionButton
          class="mt-2 shrink-0"
          :disabled="loading || disabled || !selectedProvider"
          @click="refreshModels(true)"
        >
          {{ loading ? t("imageGeneration.loading") : t("imageGeneration.refreshModels") }}
        </ToolActionButton>
      </div>
    </ToolField>
    <ToolActionButton
      v-if="catalog?.models.length"
      :disabled="disabled"
      @click="manualModel = !manualModel"
    >
      {{ manualModel ? t("imageGeneration.chooseModel") : t("imageGeneration.enterModel") }}
    </ToolActionButton>
    <p v-if="catalog?.warning" class="text-xs leading-relaxed text-amber-200">
      {{ catalog.warning }}
    </p>
    <p v-if="error" role="alert" class="text-sm text-red-300">{{ error }}</p>
    <details class="rounded-xl border border-white/10 bg-white/5 p-4">
      <summary class="text-brand cursor-pointer text-sm font-medium">
        {{ t("imageGeneration.advanced") }}
      </summary>
      <div class="mt-4 grid gap-4 sm:grid-cols-2">
        <ToolField v-if="!isNovelAi" :label="t('imageGeneration.size')">
          <ToolInput
            :model-value="modelValue.size"
            :disabled="disabled"
            @update:model-value="update('size', String($event))"
          />
        </ToolField>
        <ToolField v-if="!isNovelAi" :label="t('imageGeneration.quality')">
          <ToolInput
            :model-value="modelValue.quality"
            :disabled="disabled"
            @update:model-value="update('quality', String($event))"
          />
        </ToolField>
        <ToolField :label="t('imageGeneration.timeout')">
          <ToolInput
            type="number"
            min="30"
            max="900"
            :model-value="modelValue.timeout_secs"
            :disabled="disabled"
            @update:model-value="update('timeout_secs', Number($event))"
          />
        </ToolField>
        <ToolField :label="t('imageGeneration.catalogUrl')">
          <ToolInput
            :model-value="
              isNovelAi ? (modelValue.novelai?.models_url ?? '') : modelValue.models_url
            "
            :disabled="disabled"
            @update:model-value="updateCatalogUrl(String($event))"
          />
        </ToolField>
      </div>
      <NovelAiConfig
        v-if="isNovelAi && modelValue.novelai"
        :model-value="modelValue.novelai"
        :disabled="disabled"
        advanced
        class="mt-4"
        @update:model-value="update('novelai', $event)"
      />
      <p class="mt-3 text-xs leading-relaxed text-white/50">
        {{ t("imageGeneration.advancedHint") }}
      </p>
    </details>
  </div>
</template>
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import ToolField from "@/components/settings/tools/fields/ToolField.vue";
import ToolInput from "@/components/settings/tools/fields/ToolInput.vue";
import ToolSelect from "@/components/settings/tools/fields/ToolSelect.vue";
import ToolActionButton from "@/components/settings/tools/ToolActionButton.vue";
import NovelAiConfig from "./NovelAiConfig.vue";
import {
  listImageModels,
  listImageProviders,
  type ImageGenerationSettings,
  type ImageModelCatalog,
  type ImageProviderInfo,
} from "@/api/services/image-generation";
const props = defineProps<{
  modelValue: ImageGenerationSettings;
  disabled?: boolean;
  hideNegativeTags?: boolean;
  hideProvider?: boolean;
  providers?: ImageProviderInfo[];
}>();
const emit = defineEmits<{ "update:modelValue": [value: ImageGenerationSettings] }>();
const { t } = useI18n();
const fetchedProviders = ref<ImageProviderInfo[]>([]);
const providers = computed(() => props.providers ?? fetchedProviders.value);
const catalog = ref<ImageModelCatalog | null>(null);
const error = ref("");
const loading = ref(false);
const manualModel = ref(false);
const selectedProvider = computed(() =>
  providers.value.find((p) => p.id === props.modelValue.provider),
);
const isNovelAi = computed(() => selectedProvider.value?.protocol === "novelai");
let revision = 0;
let refreshTimer: ReturnType<typeof setTimeout> | undefined;
function update<K extends keyof ImageGenerationSettings>(
  key: K,
  value: ImageGenerationSettings[K],
) {
  emit("update:modelValue", { ...props.modelValue, [key]: value });
}
function changeProvider(provider: string) {
  manualModel.value = false;
  emit("update:modelValue", { ...props.modelValue, provider, model: "", models_url: "" });
}
function updateCatalogUrl(value: string) {
  if (isNovelAi.value && props.modelValue.novelai)
    update("novelai", { ...props.modelValue.novelai, models_url: value });
  else update("models_url", value);
}
async function refreshModels(force: boolean) {
  clearTimeout(refreshTimer);
  if (!selectedProvider.value) {
    error.value = t("imageGeneration.providerUnavailable");
    return;
  }
  const request = ++revision;
  loading.value = true;
  error.value = "";
  try {
    const result = await listImageModels({ ...props.modelValue }, force);
    if (request !== revision) return;
    catalog.value = result;
    if (
      (!result.models.length && !result.allow_automatic) ||
      (props.modelValue.model && !result.models.includes(props.modelValue.model))
    )
      manualModel.value = true;
  } catch (cause) {
    if (request === revision) {
      error.value = String(cause);
      manualModel.value = true;
    }
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
    selectedProvider.value?.id,
    providers.value,
  ],
  () => {
    revision++;
    clearTimeout(refreshTimer);
    catalog.value = null;
    error.value = "";
    loading.value = false;
    if (providers.value.length && !selectedProvider.value) {
      error.value = t("imageGeneration.providerUnavailable");
      manualModel.value = true;
      return;
    }
    if (
      selectedProvider.value &&
      (!selectedProvider.value.custom || props.modelValue.base_url.trim())
    )
      refreshTimer = setTimeout(() => void refreshModels(false), 400);
  },
);
watch(
  () => props.modelValue.provider,
  () => {
    manualModel.value = false;
  },
);
onMounted(async () => {
  try {
    if (props.providers === undefined) fetchedProviders.value = await listImageProviders();
    if (
      selectedProvider.value &&
      (!selectedProvider.value.custom || props.modelValue.base_url.trim())
    )
      await refreshModels(false);
  } catch (cause) {
    error.value = String(cause);
  }
});
onBeforeUnmount(() => {
  revision++;
  clearTimeout(refreshTimer);
});
</script>

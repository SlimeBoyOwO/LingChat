<template>
  <div class="flex min-w-0 flex-col gap-4 sm:flex-row">
    <SettingsSidebar
      :model-value="modelValue.image_generation.provider"
      :tabs="providerTabs"
      :label="t('imageGeneration.provider')"
      @update:model-value="changeProvider"
    />
    <ImageGenerationConfig
      class="min-w-0 flex-1"
      :model-value="modelValue.image_generation"
      :providers="providers"
      hide-provider
      @update:model-value="update"
    />
  </div>
</template>
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ToolSettings } from "@/api/services/tool-settings";
import {
  listImageProviders,
  type ImageGenerationSettings,
  type ImageProviderInfo,
} from "@/api/services/image-generation";
import ImageGenerationConfig from "@/components/settings/background/ImageGenerationConfig.vue";
import SettingsSidebar from "@/components/ui/SettingsSidebar.vue";
const props = defineProps<{ modelValue: ToolSettings }>();
const emit = defineEmits<{ "update:modelValue": [value: ToolSettings] }>();
const { t } = useI18n();
const providers = ref<ImageProviderInfo[]>([]);
const providerTabs = computed(() =>
  providers.value.map((provider) => ({ id: provider.id, label: provider.label })),
);
function update(image_generation: ImageGenerationSettings) {
  emit("update:modelValue", { ...props.modelValue, image_generation });
}
function changeProvider(provider: string) {
  if (provider !== props.modelValue.image_generation.provider)
    update({ ...props.modelValue.image_generation, provider, model: "", models_url: "" });
}
onMounted(async () => {
  providers.value = await listImageProviders();
});
</script>

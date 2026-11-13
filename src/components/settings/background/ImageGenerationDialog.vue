<template>
  <Teleport to="body">
    <Transition name="modal" appear>
      <div
        class="fixed inset-0 z-9999 flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm"
        @click.self="close"
        @keydown.esc.stop.prevent="close"
      >
        <SettingsModalLayout
          :title="t('imageGeneration.title')"
          :subtitle="t('imageGeneration.providerSettings')"
          :busy="busy"
          @close="close"
        >
          <template #icon><ImagePlus :size="22" /></template>
          <template #sidebar>
            <SettingsSidebar
              :model-value="settings?.provider ?? ''"
              :tabs="providerTabs"
              :label="t('imageGeneration.provider')"
              :disabled="busy || !settings"
              @update:model-value="changeProvider"
            />
          </template>
          <div class="space-y-4">
            <ImageGenerationConfig
              v-if="settings"
              v-model="settings"
              :disabled="busy"
              :providers="providers"
              hide-provider
              hide-negative-tags
            />
            <ToolField :label="t('imageGeneration.name')">
              <ToolInput v-model="name" :disabled="busy" />
            </ToolField>
            <ToolField :label="t('imageGeneration.referenceImage')">
              <input
                type="file"
                accept="image/png,image/jpeg,image/webp"
                class="glass-input mt-2"
                :disabled="busy"
                @change="loadReference"
              />
            </ToolField>
            <div
              v-if="referenceImage"
              class="flex items-center gap-3 rounded-xl border border-white/10 bg-white/5 p-3"
            >
              <img
                :src="referenceImage"
                :alt="t('imageGeneration.referenceImage')"
                class="h-20 w-32 rounded-lg object-contain"
              />
              <ToolActionButton :disabled="busy" @click="referenceImage = null">{{
                t("imageGeneration.removeReference")
              }}</ToolActionButton>
            </div>
            <ToolField
              :label="t(isNovelAi ? 'imageGeneration.positiveTags' : 'imageGeneration.prompt')"
              :hint="isNovelAi ? t('imageGeneration.tagsHint') : undefined"
            >
              <ToolTextarea
                v-model="prompt"
                :rows="4"
                :disabled="busy"
                :placeholder="
                  t(isNovelAi ? 'imageGeneration.positiveTagsHint' : 'imageGeneration.promptHint')
                "
              />
            </ToolField>
            <ToolField
              v-if="isNovelAi && settings?.novelai"
              :label="t('imageGeneration.negativeTags')"
              :hint="t('imageGeneration.negativeTagsHint')"
            >
              <ToolTextarea
                v-model="settings.novelai.negative_prompt"
                :rows="3"
                :disabled="busy"
                :placeholder="t('imageGeneration.negativeTagsPlaceholder')"
              />
            </ToolField>
            <ToolToggleRow
              :checked="applyBackground"
              :label="t('imageGeneration.applyBackground')"
              :disabled="busy"
              variant="bare"
              @change="applyBackground = $event"
            />
            <p v-if="error" role="alert" class="text-sm text-red-300">{{ error }}</p>
            <div v-if="result" class="rounded-xl border border-white/10 bg-white/5 p-3">
              <img
                :src="convertFileSrc(result.path)"
                :alt="result.scene.scene_name"
                class="max-h-80 w-full rounded-lg object-contain"
              />
              <p class="mt-2 text-sm text-emerald-200">
                {{ t("imageGeneration.saved", { name: result.scene.scene_name }) }}
              </p>
            </div>
          </div>
          <template #footer>
            <div class="flex flex-wrap justify-end gap-3">
              <ToolActionButton :disabled="busy || !settings" @click="save">{{
                t("imageGeneration.saveSettings")
              }}</ToolActionButton>
              <button
                class="btn-sm-primary rounded-full px-5 py-2.5 text-sm"
                :disabled="busy || !settings || !prompt.trim()"
                @click="generate"
              >
                {{ busy ? t("imageGeneration.generating") : t("imageGeneration.generate") }}
              </button>
            </div>
          </template>
        </SettingsModalLayout>
      </div>
    </Transition>
  </Teleport>
</template>
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ImagePlus } from "@lucide/vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import SettingsModalLayout from "@/components/ui/SettingsModalLayout.vue";
import SettingsSidebar from "@/components/ui/SettingsSidebar.vue";
import ToolField from "@/components/settings/tools/fields/ToolField.vue";
import ToolInput from "@/components/settings/tools/fields/ToolInput.vue";
import ToolTextarea from "@/components/settings/tools/fields/ToolTextarea.vue";
import ToolToggleRow from "@/components/settings/tools/ToolToggleRow.vue";
import ToolActionButton from "@/components/settings/tools/ToolActionButton.vue";
import { getToolSettings } from "@/api/services/tool-settings";
import {
  generateImage,
  listImageProviders,
  saveImageSettings,
  type GeneratedImage,
  type ImageGenerationSettings,
  type ImageProviderInfo,
} from "@/api/services/image-generation";
import ImageGenerationConfig from "./ImageGenerationConfig.vue";
const emit = defineEmits<{ close: []; generated: [result: GeneratedImage] }>();
const { t } = useI18n();
const settings = ref<ImageGenerationSettings | null>(null);
const providers = ref<ImageProviderInfo[]>([]);
const providerTabs = computed(() =>
  providers.value.map((provider) => ({ id: provider.id, label: provider.label })),
);
const isNovelAi = computed(() => settings.value?.provider === "novelai");
const name = ref("");
const prompt = ref("");
const applyBackground = ref(false);
const result = ref<GeneratedImage | null>(null);
const error = ref("");
const busy = ref(false);
const referenceImage = ref<string | null>(null);
function close() {
  if (!busy.value) emit("close");
}
function changeProvider(provider: string) {
  if (!settings.value || busy.value || provider === settings.value.provider) return;
  settings.value = { ...settings.value, provider, model: "", models_url: "" };
  result.value = null;
  error.value = "";
}
async function loadReference(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  if (file.size > 24 * 1024 * 1024) {
    error.value = t("imageGeneration.referenceTooLarge");
    input.value = "";
    return;
  }
  try {
    referenceImage.value = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(new Error(t("imageGeneration.referenceReadFailed")));
      reader.readAsDataURL(file);
    });
    error.value = "";
  } catch (cause) {
    error.value = String(cause);
  }
  input.value = "";
}
onMounted(async () => {
  try {
    const [tools, choices] = await Promise.all([getToolSettings(), listImageProviders()]);
    settings.value = tools.image_generation;
    providers.value = choices;
  } catch (cause) {
    error.value = String(cause);
  }
});
async function save() {
  if (!settings.value || busy.value) return;
  error.value = "";
  busy.value = true;
  try {
    await saveImageSettings({ ...settings.value });
  } catch (cause) {
    error.value = String(cause);
  } finally {
    busy.value = false;
  }
}
async function generate() {
  if (!settings.value || busy.value) return;
  busy.value = true;
  error.value = "";
  result.value = null;
  try {
    await saveImageSettings({ ...settings.value });
    result.value = await generateImage(
      prompt.value.trim(),
      name.value.trim(),
      applyBackground.value,
      referenceImage.value,
      isNovelAi.value ? (settings.value.novelai?.negative_prompt ?? "") : null,
    );
    emit("generated", result.value);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-9998 flex items-center justify-center bg-black/50 p-4 backdrop-blur-md"
      @click.self="!busy && emit('close')"
    >
      <section
        role="dialog"
        aria-modal="true"
        :aria-label="t('imageGeneration.title')"
        class="flex max-h-[92dvh] w-full max-w-3xl flex-col overflow-hidden rounded-2xl border border-white/20 bg-slate-900/95 text-white shadow-2xl"
      >
        <header class="flex items-center justify-between border-b border-white/10 px-5 py-4">
          <h3 class="font-bold">{{ t("imageGeneration.title") }}</h3>
          <button
            :disabled="busy"
            :aria-label="t('imageGeneration.close')"
            class="rounded px-3 py-1 text-white/65 disabled:opacity-30"
            @click="emit('close')"
          >
            ×
          </button>
        </header>
        <div class="space-y-4 overflow-y-auto p-5">
          <ImageGenerationConfig v-if="settings" v-model="settings" :disabled="busy" />
          <label class="flex flex-col gap-1.5 text-sm"
            >{{ t("imageGeneration.name") }}
            <input
              v-model="name"
              :disabled="busy"
              class="rounded-lg border border-white/20 bg-black/25 px-3 py-2"
            />
          </label>
          <label class="flex flex-col gap-1.5 text-sm">
            {{ t("imageGeneration.referenceImage") }}
            <input
              type="file"
              accept="image/png,image/jpeg,image/webp"
              :disabled="busy"
              @change="loadReference"
            />
          </label>
          <div
            v-if="referenceImage"
            class="flex items-center gap-3 rounded-lg border border-white/15 p-2"
          >
            <img
              :src="referenceImage"
              :alt="t('imageGeneration.referenceImage')"
              class="h-20 w-32 rounded object-contain"
            />
            <button
              :disabled="busy"
              class="rounded border border-white/20 px-3 py-1 text-sm"
              @click="referenceImage = null"
            >
              {{ t("imageGeneration.removeReference") }}
            </button>
          </div>
          <label class="flex flex-col gap-1.5 text-sm"
            >{{ t("imageGeneration.prompt") }}
            <textarea
              v-model="prompt"
              rows="4"
              :disabled="busy"
              :placeholder="t('imageGeneration.promptHint')"
              class="resize-y rounded-lg border border-white/20 bg-black/25 px-3 py-2"
            />
          </label>
          <label class="flex items-center gap-2 text-sm"
            ><input v-model="applyBackground" type="checkbox" :disabled="busy" />{{
              t("imageGeneration.applyBackground")
            }}</label
          >
          <p v-if="error" role="alert" class="text-sm text-red-300">{{ error }}</p>
          <div v-if="result" class="rounded-xl border border-white/15 p-3">
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
        <footer class="flex justify-end gap-3 border-t border-white/10 px-5 py-4">
          <button
            :disabled="busy || !settings"
            class="rounded-lg border border-white/20 px-4 py-2 disabled:opacity-40"
            @click="save"
          >
            {{ t("imageGeneration.saveSettings") }}
          </button>
          <button
            :disabled="busy || !settings || !prompt.trim()"
            class="rounded-lg bg-cyan-400/80 px-5 py-2 font-semibold text-slate-950 disabled:opacity-40"
            @click="generate"
          >
            {{ busy ? t("imageGeneration.generating") : t("imageGeneration.generate") }}
          </button>
        </footer>
      </section>
    </div>
  </Teleport>
</template>
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { getToolSettings } from "@/api/services/tool-settings";
import {
  generateImage,
  saveImageSettings,
  type GeneratedImage,
  type ImageGenerationSettings,
} from "@/api/services/image-generation";
import ImageGenerationConfig from "./ImageGenerationConfig.vue";
const emit = defineEmits<{ close: []; generated: [result: GeneratedImage] }>();
const { t } = useI18n();
const settings = ref<ImageGenerationSettings | null>(null);
const name = ref("");
const prompt = ref("");
const applyBackground = ref(false);
const result = ref<GeneratedImage | null>(null);
const error = ref("");
const busy = ref(false);
const referenceImage = ref<string | null>(null);
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
    settings.value = (await getToolSettings()).image_generation;
  } catch (cause) {
    error.value = String(cause);
  }
});
async function save() {
  if (!settings.value) return;
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
    );
    emit("generated", result.value);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <section class="space-y-4 rounded-xl border border-white/10 bg-white/5 p-4">
    <h4 class="text-sm font-semibold text-white/80">{{ t("settings.voicePreview.title") }}</h4>
    <p class="text-xs leading-relaxed text-white/50">{{ t("settings.voicePreview.hint") }}</p>
    <div v-if="modelValue.tts_type === 'gsv'" class="space-y-3 rounded-lg bg-black/15 p-3">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h5 class="text-sm text-white/75">{{ t("settings.voicePreview.reference") }}</h5>
        <button
          type="button"
          class="voice-button"
          :disabled="referenceBusy"
          @click="chooseReference"
        >
          {{ t("settings.voicePreview.chooseFile") }}
        </button>
      </div>
      <p class="text-xs break-all text-white/60">
        {{ referencePath || t("settings.voicePreview.noReference") }}
      </p>
      <p class="text-xs leading-relaxed text-white/45">
        {{ t("settings.voicePreview.serverPathHint") }}
      </p>
      <button
        type="button"
        class="voice-button"
        :disabled="!referencePath || referenceBusy"
        @click="loadReference"
      >
        {{
          t(referenceBusy ? "settings.voicePreview.reading" : "settings.voicePreview.loadReference")
        }}
      </button>
      <audio
        v-if="referenceUrl"
        ref="referenceAudio"
        :key="referenceUrl"
        class="h-10 w-full"
        controls
        preload="metadata"
        :src="referenceUrl"
        @loadedmetadata="referenceDuration = audioDuration($event)"
        @error="referenceError = t('settings.voicePreview.unplayable')"
      />
      <p v-if="referenceUrl" class="text-xs text-white/60">
        {{
          t("settings.voicePreview.audioInfo", {
            size: formatBytes(referenceBytes),
            duration: formatDuration(referenceDuration),
          })
        }}
      </p>
      <p v-if="referenceError" role="alert" class="text-xs leading-relaxed text-red-200">
        {{ referenceError }}
      </p>
    </div>
    <label class="block space-y-2 text-sm text-white/65">
      <span>{{ t("settings.voicePreview.text") }}</span>
      <textarea
        v-model="previewText"
        rows="3"
        maxlength="500"
        class="voice-control w-full resize-y"
        :placeholder="t('settings.voicePreview.textPlaceholder')"
      />
    </label>
    <div class="flex flex-wrap items-end gap-3">
      <label class="flex min-w-32 flex-col gap-2 text-xs text-white/65">
        <span>{{ t("settings.voicePreview.emotion") }}</span>
        <select v-model="emotion" class="voice-control">
          <option v-for="name in emotions" :key="name" :value="name">{{ name }}</option>
        </select>
      </label>
      <button
        type="button"
        class="voice-button bg-cyan-400/15! text-cyan-100!"
        :disabled="busy || !modelValue.tts_type || !previewText.trim()"
        @click="synthesize"
      >
        {{ t(busy ? "settings.voicePreview.generating" : "settings.voicePreview.generate") }}
      </button>
      <span class="ml-auto text-xs text-white/40">{{ Array.from(previewText).length }} / 500</span>
    </div>
    <p class="text-xs text-white/45">{{ t("settings.voicePreview.languageHint") }}</p>
    <p v-if="stale" class="text-xs text-amber-100/75">{{ t("settings.voicePreview.stale") }}</p>
    <p v-if="errorMessage" role="alert" class="text-xs break-words text-red-200">
      {{ errorMessage }}
    </p>
    <div v-if="previewUrl" class="space-y-2 rounded-lg bg-black/15 p-3">
      <audio
        :key="previewUrl"
        ref="previewAudio"
        class="h-10 w-full"
        controls
        preload="metadata"
        :src="previewUrl"
        @loadedmetadata="previewDuration = audioDuration($event)"
        @error="errorMessage = t('settings.voicePreview.unplayable')"
      />
      <p class="text-xs text-white/60">
        {{
          t("settings.voicePreview.audioInfo", {
            size: formatBytes(previewBytes),
            duration: formatDuration(previewDuration),
          })
        }}
      </p>
    </div>
    <p v-if="!modelValue.tts_type" class="text-xs text-white/50">
      {{ t("settings.voicePreview.selectService") }}
    </p>
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, toRaw, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { previewCharacterVoice, readCharacterReferenceAudio } from "@/api/services/character";

const props = defineProps<{ roleId: number; modelValue: Record<string, any> }>();
const emit = defineEmits<{ "reference-selected": [] }>();
const { t } = useI18n();
const previewText = ref("");
const emotion = ref("正常");
const emotions = ["正常", "高兴", "伤心", "生气", "害怕", "厌恶", "惊讶", "平静", "心动"];
const busy = ref(false);
const referenceBusy = ref(false);
const errorMessage = ref("");
const referenceError = ref("");
const previewAudio = ref<HTMLAudioElement | null>(null);
const referenceAudio = ref<HTMLAudioElement | null>(null);
const previewUrl = ref("");
const referenceUrl = ref("");
const previewBytes = ref(0);
const referenceBytes = ref(0);
const previewDuration = ref<number | null>(null);
const referenceDuration = ref<number | null>(null);
const generatedFor = ref("");
let sequence = 0;
let referenceSequence = 0;
let disposed = false;
const referencePath = computed(() =>
  String(props.modelValue.voice_models?.gsv_voice_filename ?? ""),
);
const signature = computed(() =>
  JSON.stringify([
    props.roleId,
    props.modelValue.tts_type,
    props.modelValue.voice_lang,
    props.modelValue.voice_dialect,
    props.modelValue.voice_models,
    previewText.value,
    emotion.value,
  ]),
);
const stale = computed(() => Boolean(previewUrl.value && generatedFor.value !== signature.value));

function replaceUrl(target: typeof previewUrl, bytes?: ArrayBuffer | number[]) {
  (target === previewUrl ? previewAudio : referenceAudio).value?.pause();
  if (target.value) URL.revokeObjectURL(target.value);
  target.value = "";
  if (bytes) target.value = URL.createObjectURL(new Blob([new Uint8Array(bytes)]));
}
function formatBytes(size: number) {
  return `${(size / 1024).toFixed(1)} KB`;
}
function formatDuration(duration: number | null) {
  return duration === null ? "—" : `${duration.toFixed(2)} s`;
}
function audioDuration(event: Event) {
  const value = (event.target as HTMLAudioElement).duration;
  return Number.isFinite(value) ? value : null;
}
function errorText(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

async function chooseReference() {
  const roleId = props.roleId;
  try {
    const path = await open({
      multiple: false,
      filters: [
        {
          name: t("settings.voicePreview.reference"),
          extensions: ["wav", "mp3", "flac", "ogg", "m4a", "aac"],
        },
      ],
    });
    if (
      typeof path !== "string" ||
      disposed ||
      props.roleId !== roleId ||
      props.modelValue.tts_type !== "gsv"
    )
      return;
    props.modelValue.voice_models ??= {};
    props.modelValue.voice_models.gsv_voice_filename = path;
    emit("reference-selected");
    await nextTick();
    await loadReference();
  } catch (error) {
    referenceError.value = errorText(error);
  }
}
async function loadReference() {
  const token = ++referenceSequence;
  const path = referencePath.value;
  referenceBusy.value = true;
  referenceError.value = "";
  replaceUrl(referenceUrl);
  referenceDuration.value = null;
  try {
    const bytes = await readCharacterReferenceAudio(path);
    if (disposed || token !== referenceSequence || path !== referencePath.value) return;
    referenceBytes.value = new Uint8Array(bytes).byteLength;
    replaceUrl(referenceUrl, bytes);
  } catch (error) {
    if (!disposed && token === referenceSequence) referenceError.value = errorText(error);
  } finally {
    if (!disposed && token === referenceSequence) referenceBusy.value = false;
  }
}
async function synthesize() {
  const token = ++sequence;
  const snapshot = {
    ai_name: props.modelValue.ai_name ?? "",
    tts_type: props.modelValue.tts_type,
    voice_lang: props.modelValue.voice_lang,
    voice_dialect: props.modelValue.voice_dialect,
    voice_models: structuredClone(toRaw(props.modelValue.voice_models ?? {})),
  };
  const requestedFor = signature.value;
  busy.value = true;
  errorMessage.value = "";
  replaceUrl(previewUrl);
  previewDuration.value = null;
  try {
    const bytes = await previewCharacterVoice(
      props.roleId,
      snapshot,
      previewText.value,
      emotion.value,
    );
    if (disposed || token !== sequence) return;
    previewBytes.value = new Uint8Array(bytes).byteLength;
    generatedFor.value = requestedFor;
    replaceUrl(previewUrl, bytes);
  } catch (error) {
    if (!disposed && token === sequence) errorMessage.value = errorText(error);
  } finally {
    if (!disposed && token === sequence) busy.value = false;
  }
}
watch(referencePath, () => {
  ++referenceSequence;
  referenceBusy.value = false;
  referenceError.value = "";
  replaceUrl(referenceUrl);
});
watch(
  () => props.roleId,
  () => {
    ++sequence;
    ++referenceSequence;
    busy.value = false;
    referenceBusy.value = false;
    errorMessage.value = "";
    referenceError.value = "";
    replaceUrl(previewUrl);
    replaceUrl(referenceUrl);
  },
);
onBeforeUnmount(() => {
  disposed = true;
  ++sequence;
  ++referenceSequence;
  replaceUrl(previewUrl);
  replaceUrl(referenceUrl);
});
</script>

<style scoped>
.voice-button {
  border-radius: 0.5rem;
  background: rgb(255 255 255 / 0.1);
  padding: 0.5rem 0.75rem;
  color: rgb(255 255 255 / 0.8);
  font-size: 0.75rem;
}
.voice-button:hover:not(:disabled) {
  background: rgb(255 255 255 / 0.2);
}
.voice-button:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}
.voice-control {
  border: 1px solid rgb(255 255 255 / 0.1);
  border-radius: 0.5rem;
  background: rgb(0 0 0 / 0.2);
  padding: 0.5rem 0.65rem;
  color: white;
  font-size: 0.8rem;
}
.voice-control option {
  background: #292929;
}
</style>

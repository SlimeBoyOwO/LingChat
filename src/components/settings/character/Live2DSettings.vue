<template>
  <div class="space-y-5">
    <div class="flex flex-wrap gap-2">
      <button
        v-if="!isAndroid()"
        type="button"
        class="rounded-lg bg-white/10 px-3 py-2 text-sm hover:bg-white/20"
        @click="pickSource('directory')"
      >
        {{ t("settings.characterInfo.live2d.importDirectory") }}
      </button>
      <button
        type="button"
        class="rounded-lg bg-white/10 px-3 py-2 text-sm hover:bg-white/20"
        @click="pickSource('zip')"
      >
        {{ t("settings.characterInfo.live2d.importZip") }}
      </button>
      <button
        v-if="localSettings"
        type="button"
        class="rounded-lg bg-red-500/15 px-3 py-2 text-sm text-red-200 hover:bg-red-500/30"
        @click="removeLive2d"
      >
        {{ t("settings.characterInfo.live2d.remove") }}
      </button>
    </div>

    <p v-if="busy" class="text-sm text-cyan-200">
      {{ t("settings.characterInfo.live2d.loading") }}
    </p>
    <p v-if="errorMessage" class="text-sm text-red-300">
      {{ errorMessage }}
    </p>
    <p v-if="!localSettings && !busy" class="py-8 text-center text-sm text-white/45">
      {{ t("settings.characterInfo.live2d.empty") }}
    </p>

    <template v-if="localSettings">
      <div class="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1fr)_280px]">
        <div class="space-y-4">
          <label class="flex flex-col gap-2 text-sm text-white/70">
            {{ t("settings.characterInfo.live2d.defaultVariant") }}
            <select
              :value="localSettings.default_variant"
              class="live2d-control"
              @change="setDefaultVariant(($event.target as HTMLSelectElement).value)"
            >
              <option v-for="name in variantNames" :key="name" :value="name">
                {{ name }}
              </option>
            </select>
          </label>

          <label class="flex flex-col gap-2 text-sm text-white/70">
            {{ t("settings.characterInfo.live2d.editVariant") }}
            <select v-model="selectedVariant" class="live2d-control">
              <option v-for="name in variantNames" :key="name" :value="name">
                {{ name }}
              </option>
            </select>
          </label>

          <div
            v-if="currentVariant"
            class="space-y-3 rounded-lg border border-white/10 bg-white/5 p-3"
          >
            <div class="text-xs break-all text-white/55">{{ currentVariant.model }}</div>
            <label class="flex flex-col gap-2 text-sm text-white/70">
              {{ t("settings.characterInfo.live2d.defaultExpression") }}
              <select v-model="currentVariant.default_expression" class="live2d-control">
                <option value="">-</option>
                <option v-for="name in expressionOptions" :key="name" :value="name">
                  {{ name }}
                </option>
              </select>
              <span class="text-xs text-white/45">
                {{ t("settings.characterInfo.live2d.defaultExpressionHint") }}
              </span>
            </label>
            <div class="grid grid-cols-2 gap-2">
              <label class="flex flex-col gap-2 text-sm text-white/70">
                {{ t("settings.characterInfo.live2d.focusAnchorX") }}
                <input
                  type="number"
                  min="0"
                  max="1"
                  step="0.01"
                  class="live2d-control"
                  :value="currentVariant.focus_anchor?.x ?? 0.5"
                  @input="setFocusAnchor('x', ($event.target as HTMLInputElement).value)"
                />
              </label>
              <label class="flex flex-col gap-2 text-sm text-white/70">
                {{ t("settings.characterInfo.live2d.focusAnchorY") }}
                <input
                  type="number"
                  min="0"
                  max="1"
                  step="0.01"
                  class="live2d-control"
                  :value="currentVariant.focus_anchor?.y ?? 0.5"
                  @input="setFocusAnchor('y', ($event.target as HTMLInputElement).value)"
                />
              </label>
            </div>
            <button
              v-if="currentVariant.focus_anchor"
              type="button"
              class="text-left text-xs text-white/50 hover:text-white/80"
              @click="currentVariant.focus_anchor = null"
            >
              {{ t("settings.characterInfo.live2d.focusAnchorReset") }}
            </button>
            <div class="grid grid-cols-1 gap-2 md:grid-cols-2">
              <div v-for="emotion in emotions" :key="emotion" class="rounded-lg bg-black/15 p-2">
                <div
                  class="mb-2 flex items-center justify-between gap-2 text-xs font-medium text-white/70"
                >
                  <span>{{ emotion }}</span>
                  <button
                    type="button"
                    class="preview-button"
                    :disabled="!previewReady"
                    @click="testEmotion(emotion)"
                  >
                    {{ t("settings.live2dPreview.test") }}
                  </button>
                </div>
                <select v-model="currentVariant.expressions[emotion]" class="live2d-control mb-2">
                  <option value="">{{ t("settings.characterInfo.live2d.noExpression") }}</option>
                  <option v-for="name in expressionOptions" :key="name" :value="name">
                    {{ name }}
                  </option>
                </select>
                <select
                  class="live2d-control"
                  :value="motionValue(emotion)"
                  @change="setMotion(emotion, ($event.target as HTMLSelectElement).value)"
                >
                  <option value="">{{ t("settings.characterInfo.live2d.noMotion") }}</option>
                  <option v-for="motion in motionOptions" :key="motion.value" :value="motion.value">
                    {{ motion.label }}
                  </option>
                </select>
              </div>
            </div>
          </div>

          <div
            v-if="currentVariant"
            class="space-y-2 rounded-lg border border-white/10 bg-white/5 p-3"
          >
            <h4 class="text-sm font-semibold text-white/75">
              {{ t("settings.characterInfo.live2d.touchReactions") }}
            </h4>
            <p class="text-xs text-white/45">
              {{ t("settings.characterInfo.live2d.touchHint") }}
            </p>
            <div class="grid grid-cols-1 gap-2 md:grid-cols-2">
              <div v-for="part in touchParts" :key="part" class="rounded-lg bg-black/15 p-2">
                <label class="mb-2 flex items-center gap-2 text-xs font-medium text-white/70">
                  <input
                    type="checkbox"
                    :checked="touchEnabled(part)"
                    @change="setTouchEnabled(part, ($event.target as HTMLInputElement).checked)"
                  />
                  {{ touchPartLabel(part) }}
                </label>
                <button
                  type="button"
                  class="preview-button mb-2"
                  :disabled="!previewReady || !touchEnabled(part)"
                  @click="runPreview(() => stageRef?.previewTouch(roleId, part))"
                >
                  {{ t("settings.live2dPreview.testTouch") }}
                </button>
                <select
                  class="live2d-control mb-2"
                  :disabled="!touchEnabled(part)"
                  :value="touchExpression(part)"
                  @change="setTouchExpression(part, ($event.target as HTMLSelectElement).value)"
                >
                  <option value="">{{ t("settings.characterInfo.live2d.noExpression") }}</option>
                  <option v-for="name in expressionOptions" :key="name" :value="name">
                    {{ name }}
                  </option>
                </select>
                <select
                  class="live2d-control"
                  :disabled="!touchEnabled(part)"
                  :value="touchMotionValue(part)"
                  @change="setTouchMotion(part, ($event.target as HTMLSelectElement).value)"
                >
                  <option value="">{{ t("settings.characterInfo.live2d.noMotion") }}</option>
                  <option v-for="motion in motionOptions" :key="motion.value" :value="motion.value">
                    {{ motion.label }}
                  </option>
                </select>
              </div>
            </div>
          </div>

          <div
            v-if="clothesNames.length"
            class="space-y-2 rounded-lg border border-white/10 bg-white/5 p-3"
          >
            <h4 class="text-sm font-semibold text-white/75">
              {{ t("settings.characterInfo.live2d.clothesMapping") }}
            </h4>
            <label
              v-for="clothes in clothesNames"
              :key="clothes"
              class="grid grid-cols-2 items-center gap-3 text-sm"
            >
              <span>{{ clothes }}</span>
              <select v-model="localSettings.clothes_variants[clothes]" class="live2d-control">
                <option value="">{{ localSettings.default_variant }}</option>
                <option v-for="name in variantNames" :key="name" :value="name">
                  {{ name }}
                </option>
              </select>
            </label>
          </div>
        </div>

        <aside
          class="order-first min-w-0 space-y-3 self-start rounded-xl border border-white/10 bg-black/20 p-3 xl:sticky xl:top-0 xl:order-last xl:max-h-[calc(85dvh-12rem)] xl:overflow-y-auto"
        >
          <h4 class="text-sm font-semibold text-white/75">
            {{ t("settings.live2dPreview.title") }}
          </h4>
          <div
            class="relative h-72 overflow-hidden rounded-lg border border-white/10 bg-black/25"
            :class="{ 'cursor-crosshair': pickingAnchor }"
            @click="pickAnchor"
          >
            <Live2DStage
              v-if="previewRole"
              :key="`${roleId}:${selectedVariant}`"
              ref="stageRef"
              class="relative! h-full w-full"
              :roles="[previewRole]"
              mode="standard"
              editor-preview
              :active-speaker-id="null"
              :audio-element="null"
              voice-data-url=""
              @active-change="onPreviewActive"
              @failed-change="onPreviewFailed"
              @preview-geometry="anchorPosition = $event"
            />
            <span
              v-if="previewReady && anchorPosition"
              class="pointer-events-none absolute -translate-x-1/2 -translate-y-1/2 text-xl leading-none text-cyan-200"
              :style="{ left: `${anchorPosition.x * 100}%`, top: `${anchorPosition.y * 100}%` }"
              aria-hidden="true"
              >⊕</span
            >
            <span
              v-if="!previewReady"
              class="pointer-events-none absolute inset-x-2 bottom-2 text-center text-xs text-white/50"
              >{{
                previewFailed
                  ? t("settings.live2dPreview.loadFailed")
                  : t("settings.live2dPreview.loading")
              }}</span
            >
          </div>
          <button
            type="button"
            class="preview-button w-full"
            :class="{ 'bg-cyan-400/20!': pickingAnchor }"
            :disabled="!previewReady"
            @click="pickingAnchor = !pickingAnchor"
          >
            {{
              t(
                pickingAnchor
                  ? "settings.live2dPreview.cancelPick"
                  : "settings.live2dPreview.pickAnchor",
              )
            }}
          </button>
          <p class="text-xs leading-relaxed text-white/45">
            {{ t("settings.live2dPreview.anchorHint") }}
          </p>
          <label class="block space-y-2 text-xs text-white/65">
            <span>{{ t("settings.live2dPreview.emotion") }}</span>
            <select
              v-model="previewEmotionValue"
              class="live2d-control"
              @change="testEmotion(previewEmotionValue)"
            >
              <option v-for="emotion in emotions" :key="emotion" :value="emotion">
                {{ emotion }}
              </option>
            </select>
          </label>
          <button
            type="button"
            class="preview-button w-full"
            :disabled="!previewReady"
            @click="testEmotion(previewEmotionValue)"
          >
            {{ t("settings.live2dPreview.replay") }}
          </button>
          <label class="block space-y-2 text-xs text-white/65">
            <span>{{ t("settings.live2dPreview.expression") }}</span>
            <select v-model="testExpression" class="live2d-control">
              <option value="">{{ t("settings.live2dPreview.chooseExpression") }}</option>
              <option v-for="name in expressionOptions" :key="name" :value="name">
                {{ name }}
              </option>
            </select>
          </label>
          <button
            type="button"
            class="preview-button w-full"
            :disabled="!previewReady || !testExpression"
            @click="runPreview(() => stageRef?.previewExpression(roleId, testExpression))"
          >
            {{ t("settings.live2dPreview.applyExpression") }}
          </button>
          <label class="block space-y-2 text-xs text-white/65">
            <span>{{ t("settings.live2dPreview.motion") }}</span>
            <select v-model="testMotion" class="live2d-control">
              <option value="">{{ t("settings.live2dPreview.chooseMotion") }}</option>
              <option v-for="motion in motionOptions" :key="motion.value" :value="motion.value">
                {{ motion.label }}
              </option>
            </select>
          </label>
          <button
            type="button"
            class="preview-button w-full"
            :disabled="!previewReady || !testMotion"
            @click="playSelectedMotion"
          >
            {{ t("settings.live2dPreview.playMotion") }}
          </button>
          <p role="status" class="min-h-8 text-xs leading-relaxed text-cyan-100/70">
            {{ previewStatus || t("settings.live2dPreview.hint") }}
          </p>
        </aside>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { useCharacterEditorApi } from "@/composables/useCharacterEditor";
const { importLive2d, inspectLive2d } = useCharacterEditorApi();
import { open } from "@tauri-apps/plugin-dialog";
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import Live2DStage from "@/components/game/live2d/Live2DStage.vue";
import { TOUCH_PART_ORDER } from "@/components/game/live2d/live2d-touch";
import type { GameRole } from "@/stores/modules/game/state";
import type { Live2dImportResult, Live2dSettings } from "@/types/live2d";
import { isAndroid } from "@/utils/platform";

const props = defineProps<{
  roleId: number;
  characterFolder: string;
  clothes: Array<{ name?: string }> | null | undefined;
  scale?: number;
  offsetX?: number;
  offsetY?: number;
  modelValue: Live2dSettings | null | undefined;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: Live2dSettings | null];
}>();

const { t } = useI18n();
const localSettings = computed<Live2dSettings | null>({
  get: () => props.modelValue ?? null,
  set: (value) => emit("update:modelValue", value),
});
const metadata = ref<Live2dImportResult["models"]>([]);
const selectedVariant = ref("");
const stageRef = ref<InstanceType<typeof Live2DStage> | null>(null);
const previewReady = ref(false);
const previewFailed = ref(false);
const previewEmotionValue = ref("正常");
const testExpression = ref("");
const testMotion = ref("");
const previewStatus = ref("");
const pickingAnchor = ref(false);
const anchorPosition = ref<{ x: number; y: number } | null>(null);
let previewSequence = 0;
const busy = ref(false);
const errorMessage = ref("");

const emotions = [
  "正常",
  "平静",
  "高兴",
  "兴奋",
  "生气",
  "害羞",
  "疑惑",
  "哭泣",
  "惊讶",
  "厌恶",
  "担心",
  "认真",
  "紧张",
  "害怕",
  "慌张",
  "无奈",
  "心动",
  "调皮",
  "难为情",
  "自信",
];

const variantNames = computed(() => Object.keys(localSettings.value?.variants ?? {}));
const currentVariant = computed(() => localSettings.value?.variants[selectedVariant.value]);
const currentMetadata = computed(() =>
  metadata.value.find((item) => item.variant === selectedVariant.value),
);
const expressionOptions = computed(() => currentMetadata.value?.expressions ?? []);
const motionOptions = computed(() => {
  const options: Array<{ value: string; label: string }> = [];
  for (const [group, files] of Object.entries(currentMetadata.value?.motions ?? {})) {
    files.forEach((file, index) =>
      options.push({ value: `${group}:${index}`, label: `${group}[${index}] ${file}` }),
    );
  }
  return options;
});
const clothesNames = computed(() => [
  "default",
  ...(props.clothes ?? [])
    .map((item) => item.name?.trim())
    .filter((name): name is string => Boolean(name)),
]);

const previewRole = computed<GameRole | null>(() => {
  if (!localSettings.value || !selectedVariant.value) return null;
  const previewSettings: Live2dSettings = {
    ...localSettings.value,
    default_variant: selectedVariant.value,
    clothes_variants: { default: selectedVariant.value },
  };
  return {
    roleId: props.roleId,
    roleName: "",
    roleSubTitle: "",
    thinkMessage: "",
    emotion: previewEmotionValue.value,
    originalEmotion: previewEmotionValue.value,
    scale: props.scale ?? 1,
    offsetY: props.offsetY ?? 0,
    offsetX: props.offsetX ?? 0,
    scaleP: 1,
    offsetXP: 0,
    offsetYP: 0,
    bubbleTop: 0,
    bubbleLeft: 0,
    show: true,
    clothes: {},
    clothesName: "default",
    bodyPart: {},
    live2d: previewSettings,
    character_folder: props.characterFolder,
  };
});

watch(
  () => props.modelValue?.default_variant,
  (defaultVariant) => {
    if (!selectedVariant.value || !props.modelValue?.variants[selectedVariant.value]) {
      selectedVariant.value = defaultVariant ?? "";
    }
  },
  { immediate: true },
);

watch(
  () => props.roleId,
  async () => {
    if (!props.modelValue) return;
    try {
      metadata.value = (await inspectLive2d(props.roleId)).models;
    } catch (error) {
      console.warn("[Live2D] Failed to inspect current role models", error);
    }
  },
  { immediate: true },
);

watch([selectedVariant, () => props.roleId], () => {
  previewSequence += 1;
  previewReady.value = false;
  previewFailed.value = false;
  previewStatus.value = "";
  pickingAnchor.value = false;
  anchorPosition.value = null;
  testExpression.value = "";
  testMotion.value = "";
});

function onPreviewActive(ids: number[]) {
  const wasReady = previewReady.value;
  previewReady.value = ids.includes(props.roleId);
  if (!wasReady && previewReady.value) void testEmotion(previewEmotionValue.value);
}

function onPreviewFailed(ids: number[]) {
  previewFailed.value = ids.includes(props.roleId);
  if (previewFailed.value) previewReady.value = false;
}

async function runPreview(action: () => boolean | undefined | Promise<boolean | undefined>) {
  if (!previewReady.value) return;
  const sequence = ++previewSequence;
  try {
    await nextTick();
    if (sequence !== previewSequence || !previewReady.value) return;
    const started = await action();
    if (sequence !== previewSequence) return;
    previewStatus.value = t(
      started ? "settings.live2dPreview.started" : "settings.live2dPreview.unavailable",
    );
  } catch (error) {
    console.warn("[Live2D] Preview action failed", error);
    if (sequence !== previewSequence) return;
    previewStatus.value = t("settings.live2dPreview.unavailable");
  }
}

function testEmotion(emotion: string) {
  previewEmotionValue.value = emotion;
  return runPreview(() => stageRef.value?.previewEmotion(props.roleId, emotion));
}

function playSelectedMotion() {
  const separator = testMotion.value.lastIndexOf(":");
  const binding = {
    group: testMotion.value.slice(0, separator),
    index: Number(testMotion.value.slice(separator + 1)),
  };
  return runPreview(() => stageRef.value?.previewMotion(props.roleId, binding));
}

function pickAnchor(event: MouseEvent) {
  if (!pickingAnchor.value || !currentVariant.value) return;
  const anchor = stageRef.value?.pickFocusAnchor(props.roleId, event.clientX, event.clientY);
  if (!anchor) {
    previewStatus.value = t("settings.live2dPreview.outsideModel");
    return;
  }
  currentVariant.value.focus_anchor = anchor;
  pickingAnchor.value = false;
  previewStatus.value = t("settings.live2dPreview.anchorPicked");
}

function setFocusAnchor(axis: "x" | "y", rawValue: string) {
  const variant = currentVariant.value;
  const value = Number(rawValue);
  if (!variant || !Number.isFinite(value)) return;
  const anchor = variant.focus_anchor ?? { x: 0.5, y: 0.5 };
  variant.focus_anchor = { ...anchor, [axis]: Math.min(1, Math.max(0, value)) };
}

function setDefaultVariant(variantName: string) {
  const settings = localSettings.value;
  if (!settings?.variants[variantName]) return;
  settings.default_variant = variantName;
  if (Object.prototype.hasOwnProperty.call(settings.clothes_variants, "default")) {
    settings.clothes_variants.default = variantName;
  }
  selectedVariant.value = variantName;
}

function motionValue(emotion: string) {
  const motion = currentVariant.value?.motions[emotion];
  return motion ? `${motion.group}:${motion.index}` : "";
}

function setMotion(emotion: string, value: string) {
  const variant = currentVariant.value;
  if (!variant) return;
  if (!value) {
    delete variant.motions[emotion];
    return;
  }
  const separator = value.lastIndexOf(":");
  variant.motions[emotion] = {
    ...variant.motions[emotion],
    group: value.slice(0, separator),
    index: Number(value.slice(separator + 1)),
    loop: false,
  };
}

// --- 抚摸反应 ---

/** 判定顺序同时也是展示顺序，与运行时共用一份，免得两处各列一遍导致漏配部位。 */
const touchParts = TOUCH_PART_ORDER;

function touchBinding(part: string) {
  return currentVariant.value?.touch_motions?.[part];
}

/** 部位出现在表里就代表可摸，所以「只晃动、表情动作都留空」也是合法配置。 */
function touchEnabled(part: string) {
  return touchBinding(part) !== undefined;
}

function touchPartLabel(part: string) {
  return t(`settings.characterInfo.live2d.touchPart_${part}`);
}

function setTouchEnabled(part: string, enabled: boolean) {
  const variant = currentVariant.value;
  if (!variant) return;
  const bindings = { ...(variant.touch_motions ?? {}) };
  if (enabled) {
    if (!bindings[part]) bindings[part] = {};
  } else {
    delete bindings[part];
  }
  // 空表整个删掉而不是留个 null：Rust 侧这个字段是 HashMap 不是 Option，
  // 写出 null 下次读取会反序列化失败
  if (Object.keys(bindings).length) variant.touch_motions = bindings;
  else delete variant.touch_motions;
}

function touchExpression(part: string) {
  return touchBinding(part)?.expression ?? "";
}

function setTouchExpression(part: string, value: string) {
  const binding = touchBinding(part);
  if (!binding) return;
  if (value) binding.expression = value;
  else delete binding.expression;
}

function touchMotionValue(part: string) {
  const binding = touchBinding(part);
  if (binding?.group === undefined || binding.index === undefined) return "";
  return `${binding.group}:${binding.index}`;
}

function setTouchMotion(part: string, value: string) {
  const binding = touchBinding(part);
  if (!binding) return;
  // group 与 index 必须同进同退，只给一个运行时不会播任何动作
  if (!value) {
    delete binding.group;
    delete binding.index;
    return;
  }
  const separator = value.lastIndexOf(":");
  binding.group = value.slice(0, separator);
  binding.index = Number(value.slice(separator + 1));
}

async function pickSource(sourceKind: "directory" | "zip") {
  const selection = await open({
    directory: sourceKind === "directory",
    multiple: false,
    filters: sourceKind === "zip" ? [{ name: "Live2D ZIP", extensions: ["zip"] }] : undefined,
  });
  if (typeof selection !== "string") return;
  busy.value = true;
  errorMessage.value = "";
  try {
    const result = await importLive2d(props.roleId, selection, sourceKind);
    metadata.value = result.models;
    localSettings.value = result.live2d;
    selectedVariant.value = result.live2d.default_variant;
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    busy.value = false;
  }
}

function removeLive2d() {
  localSettings.value = null;
  metadata.value = [];
  selectedVariant.value = "";
}
</script>

<style scoped>
.preview-button {
  border-radius: 0.5rem;
  background: rgb(255 255 255 / 0.1);
  padding: 0.4rem 0.65rem;
  color: rgb(255 255 255 / 0.8);
  font-size: 0.75rem;
}
.preview-button:hover:not(:disabled) {
  background: rgb(255 255 255 / 0.2);
}
.preview-button:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.live2d-control {
  width: 100%;
  border: 1px solid rgb(255 255 255 / 0.1);
  border-radius: 0.5rem;
  background: rgb(0 0 0 / 0.2);
  padding: 0.45rem 0.65rem;
  color: white;
  font-size: 0.8rem;
}
.live2d-control option {
  background: #292929;
}
</style>

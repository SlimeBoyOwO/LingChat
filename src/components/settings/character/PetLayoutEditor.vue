<template>
  <section class="space-y-3 rounded-xl border border-white/10 bg-white/5 p-4">
    <h3 class="text-sm font-semibold text-white/80">{{ t("settings.petLayout.title") }}</h3>
    <p class="text-xs leading-relaxed text-white/50">{{ t("settings.petLayout.hint") }}</p>
    <div class="flex flex-wrap gap-2">
      <select
        v-model="costume"
        class="rounded-lg border border-white/10 bg-[#252535] px-2 py-1.5 text-xs text-white"
        :aria-label="t('settings.avatars.costume')"
      >
        <option value="default">{{ t("settings.avatars.default") }}</option>
        <option v-for="name in costumeNames" :key="name" :value="name">{{ name }}</option>
      </select>
      <select
        v-model="emotion"
        class="rounded-lg border border-white/10 bg-[#252535] px-2 py-1.5 text-xs text-white"
        :aria-label="t('settings.roleLayout.emotion')"
      >
        <option v-for="name in Object.keys(EMOTION_CONFIG_EMO)" :key="name" :value="name">
          {{ name }}
        </option>
      </select>
      <label class="flex items-center gap-2 text-xs text-white/70"
        ><input
          type="checkbox"
          :checked="modelValue.pet_frameless"
          @change="patch({ pet_frameless: ($event.target as HTMLInputElement).checked })"
        />{{ t("settings.petLayout.frameless") }}</label
      >
      <label class="flex items-center gap-2 text-xs text-white/70"
        ><input v-model="showBounds" type="checkbox" />{{ t("settings.petLayout.bounds") }}</label
      >
      <button
        type="button"
        class="rounded-lg bg-white/10 px-3 py-1.5 text-xs text-white/75 hover:bg-white/15"
        @click="patch({ scale_p: 1, offset_x_p: 0, offset_y_p: 0 })"
      >
        {{ t("settings.petLayout.reset") }}
      </button>
    </div>
    <div
      ref="frame"
      class="relative mx-auto aspect-square w-full max-w-[360px] overflow-hidden rounded-xl border border-white/10 bg-black/20"
    >
      <div
        tabindex="0"
        role="group"
        :aria-label="t('settings.petLayout.canvas')"
        class="absolute top-0 left-0 origin-top-left touch-none outline-none select-none focus-visible:ring-2 focus-visible:ring-cyan-300"
        :style="{
          width: `${logicalSize}px`,
          height: `${logicalSize}px`,
          transform: `scale(${previewScale})`,
        }"
        @pointerdown="startDrag"
        @pointermove="moveDrag"
        @pointerup="finishDrag"
        @pointercancel="finishDrag"
        @lostpointercapture="finishDrag"
        @keydown="nudge"
        @wheel.prevent="zoom"
      >
        <div
          class="pointer-events-none absolute top-6 left-6"
          :style="{ width: `${petSize}px`, height: `${petSize}px` }"
        >
          <div
            v-if="showBounds && !live2dActive && imageUrl"
            class="absolute inset-0 opacity-25"
            :style="imageTransform"
          >
            <ImageAcrossFade
              class="h-full w-full"
              :style="{ top: '-10px' }"
              :src="imageUrl"
              :duration="0"
              position="center 0%"
              object-fit="cover"
            />
          </div>
          <div
            class="absolute inset-0 overflow-hidden"
            :class="
              modelValue.pet_frameless
                ? ''
                : 'rounded-full border-2 border-white/60 bg-white/10 shadow-[0_8px_32px_rgba(0,176,255,0.15)]'
            "
          >
            <div v-if="!live2dActive" class="h-full w-full origin-top" :style="imageTransform">
              <ImageAcrossFade
                v-if="imageUrl"
                class="h-full w-full"
                :style="{ top: '-10px' }"
                :src="imageUrl"
                :duration="0"
                position="center 0%"
                object-fit="cover"
              />
            </div>
          </div>
          <Live2DStage
            v-if="live2dActive"
            :roles="[previewRole]"
            mode="pet"
            :class="modelValue.pet_frameless ? '' : 'rounded-full'"
            :active-speaker-id="null"
            :audio-element="null"
            voice-data-url=""
            @failed-change="live2dFailed = $event.includes(roleId)"
          />
          <div
            v-if="showBounds"
            class="absolute inset-0 border border-dashed border-cyan-200/50"
          ></div>
        </div>
      </div>
    </div>
    <p
      v-if="(error && !live2dActive) || live2dFailed"
      role="status"
      class="text-xs text-amber-200/80"
    >
      {{ live2dFailed ? t("settings.roleLayout.modelFailed") : error }}
    </p>
    <label class="flex items-center gap-3 text-xs text-white/60"
      >{{ t("settings.roleLayout.scale")
      }}<input
        type="range"
        min="0.1"
        max="3"
        step="0.01"
        :value="modelValue.scale_p ?? 1"
        class="min-w-0 flex-1 accent-[#79d9ff]"
        @input="patch({ scale_p: Number(($event.target as HTMLInputElement).value) })"
      /><span class="w-12 text-right tabular-nums">{{
        Number(modelValue.scale_p ?? 1).toFixed(2)
      }}</span></label
    >
    <p class="text-xs leading-relaxed text-white/40">
      {{ t("settings.petLayout.size", { size: petSize }) }}
    </p>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { listCharacterAvatars } from "@/api/services/character";
import { useSettingsStore } from "@/stores/modules/settings";
import { EMOTION_CONFIG_EMO } from "@/controllers/emotion/config";
import { prefersLive2d } from "@/types/live2d";
import type { GameRole } from "@/stores/modules/game/state";
import ImageAcrossFade from "@/components/ui/ImageAcrossFade.vue";
import Live2DStage from "@/components/game/live2d/Live2DStage.vue";

const props = defineProps<{
  roleId: number;
  modelValue: Record<string, any>;
  clothes?: Array<{ title: string }>;
}>();
const emit = defineEmits<{ "update:modelValue": [value: Record<string, any>] }>();
const { t } = useI18n();
const settings = useSettingsStore();
const petSize = computed(() => Math.round(210 * (settings.pet?.scale || 1)));
const logicalSize = computed(() => petSize.value + 48);
const frame = ref<HTMLElement>();
const frameWidth = ref(1);
const previewScale = computed(() => frameWidth.value / logicalSize.value);
let observer: ResizeObserver | undefined;
onMounted(() => {
  observer = new ResizeObserver(() => {
    frameWidth.value = frame.value?.clientWidth || 1;
  });
  if (frame.value) {
    frameWidth.value = frame.value.clientWidth;
    observer.observe(frame.value);
  }
});
onUnmounted(() => observer?.disconnect());
const costume = ref("default");
const emotion = ref("正常");
const showBounds = ref(true);
const costumeNames = computed(
  () =>
    [
      ...new Set(
        [
          ...(props.clothes ?? []).map((c) => c.title),
          ...(props.modelValue.clothes ?? []).map((c: { name: string }) => c.name),
        ].filter((n) => n && n !== "default" && n !== "默认"),
      ),
    ] as string[],
);
const imageUrl = ref("");
const error = ref("");
const live2dFailed = ref(false);
let generation = 0;
watch(
  () => [props.roleId, costume.value, emotion.value],
  async () => {
    const current = ++generation;
    error.value = "";
    try {
      const slots = await listCharacterAvatars(props.roleId, costume.value);
      if (current !== generation) return;
      const path = slots.find((s) => s.emotion === EMOTION_CONFIG_EMO[emotion.value])?.path;
      imageUrl.value = path ? `${convertFileSrc(path)}?v=${Date.now()}` : "";
      if (!path) error.value = t("settings.roleLayout.missingImage");
    } catch (e) {
      if (current === generation) {
        imageUrl.value = "";
        error.value = String(e);
      }
    }
  },
  { immediate: true },
);
const previewRole = computed<GameRole>(() => ({
  roleId: props.roleId,
  roleName: "",
  roleSubTitle: "",
  thinkMessage: "",
  emotion: emotion.value,
  originalEmotion: emotion.value,
  show: true,
  scale: 1,
  offsetX: 0,
  offsetY: 0,
  scaleP: Number(props.modelValue.scale_p ?? 1),
  offsetXP: Number(props.modelValue.offset_x_p ?? 0),
  offsetYP: Number(props.modelValue.offset_y_p ?? 0),
  bubbleTop: 0,
  bubbleLeft: 0,
  clothes: {},
  clothesName: costume.value,
  bodyPart: {},
  live2d: props.modelValue.live2d,
  avatarModeP: props.modelValue.avatar_mode_p,
  petFrameless: props.modelValue.pet_frameless,
  character_folder: props.modelValue.character_folder ?? "",
}));
const live2dActive = computed(() => prefersLive2d(previewRole.value, "pet") && !live2dFailed.value);
watch(
  () => [props.roleId, props.modelValue.avatar_mode_p, props.modelValue.live2d, costume.value],
  () => {
    live2dFailed.value = false;
  },
);
const imageTransform = computed(() => ({
  transform: `scale(${previewRole.value.scaleP}) translate(${previewRole.value.offsetXP}px, ${previewRole.value.offsetYP}px)`,
  transformOrigin: "center top",
}));
function patch(values: Record<string, number | boolean>) {
  emit("update:modelValue", { ...props.modelValue, ...values });
}
let drag:
  | { pointer: number; x: number; y: number; offsetX: number; offsetY: number; ratio: number }
  | undefined;
function startDrag(event: PointerEvent) {
  if (event.button !== 0) return;
  const target = event.currentTarget as HTMLElement;
  target.setPointerCapture(event.pointerId);
  target.focus({ preventScroll: true });
  drag = {
    pointer: event.pointerId,
    x: event.clientX,
    y: event.clientY,
    offsetX: previewRole.value.offsetXP,
    offsetY: previewRole.value.offsetYP,
    ratio: previewScale.value * (live2dActive.value ? 1 : previewRole.value.scaleP || 1),
  };
}
function moveDrag(event: PointerEvent) {
  if (drag && drag.pointer === event.pointerId)
    patch({
      offset_x_p: Math.round(drag.offsetX + (event.clientX - drag.x) / drag.ratio),
      offset_y_p: Math.round(drag.offsetY + (event.clientY - drag.y) / drag.ratio),
    });
}
function finishDrag() {
  drag = undefined;
}
function zoom(event: WheelEvent) {
  patch({
    scale_p: Math.max(
      0.1,
      Math.min(
        3,
        Math.round((previewRole.value.scaleP + (event.deltaY < 0 ? 0.05 : -0.05)) * 100) / 100,
      ),
    ),
  });
}
function nudge(event: KeyboardEvent) {
  const delta = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[
    event.key
  ];
  if (!delta) return;
  event.preventDefault();
  const step = event.shiftKey ? 10 : 1;
  patch({
    offset_x_p: previewRole.value.offsetXP + delta[0] * step,
    offset_y_p: previewRole.value.offsetYP + delta[1] * step,
  });
}
</script>

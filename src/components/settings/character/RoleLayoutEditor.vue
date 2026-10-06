<template>
  <section class="space-y-3 rounded-xl border border-white/10 bg-white/5 p-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h3 class="text-sm font-semibold text-white/80">{{ t("settings.roleLayout.title") }}</h3>
      <select
        v-model="viewport"
        class="rounded-lg border border-white/10 bg-[#252535] px-2 py-1.5 text-xs text-white"
      >
        <option value="landscape">{{ t("settings.roleLayout.landscape") }}</option>
        <option value="portrait">{{ t("settings.roleLayout.portrait") }}</option>
      </select>
    </div>
    <p class="text-xs leading-relaxed text-white/50">{{ t("settings.roleLayout.hint") }}</p>
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
      <button
        v-for="target in ['role', 'bubble'] as const"
        :key="target"
        type="button"
        :aria-pressed="selection === target"
        class="rounded-lg px-3 py-1.5 text-xs text-white/75"
        :class="selection === target ? 'bg-[#5e72e4]' : 'bg-white/10 hover:bg-white/15'"
        @click="selection = target"
      >
        {{ t(`settings.roleLayout.${target}`) }}
      </button>
      <button
        type="button"
        class="rounded-lg bg-white/10 px-3 py-1.5 text-xs text-white/75 hover:bg-white/15"
        @click="reset"
      >
        {{ t("settings.roleLayout.reset") }}
      </button>
    </div>
    <div
      ref="frame"
      class="relative mx-auto w-full max-w-full overflow-hidden rounded-xl border border-white/10 bg-[radial-gradient(ellipse_at_center,#33445c,#151a29)]"
      :style="{
        aspectRatio: `${size.width}/${size.height}`,
        maxHeight: '420px',
        width: `min(100%, ${(420 * size.width) / size.height}px)`,
      }"
    >
      <div
        class="absolute top-0 left-0 origin-top-left touch-none outline-none select-none focus-visible:ring-2 focus-visible:ring-cyan-300"
        :style="{
          width: `${size.width}px`,
          height: `${size.height}px`,
          transform: `scale(${previewScale})`,
        }"
        tabindex="0"
        role="group"
        :aria-label="t('settings.roleLayout.canvas')"
        @pointerdown="startDrag($event, selection)"
        @pointermove="moveDrag"
        @pointerup="finishDrag"
        @pointercancel="finishDrag"
        @lostpointercapture="finishDrag"
        @keydown="nudge"
        @wheel.prevent="zoom"
      >
        <div
          class="pointer-events-none absolute inset-0 opacity-20"
          style="
            background-image:
              linear-gradient(#ffffff33 1px, transparent 1px),
              linear-gradient(90deg, #ffffff33 1px, transparent 1px);
            background-size: 80px 80px;
          "
        ></div>
        <Live2DStage
          v-if="live2dActive"
          :roles="[previewRole]"
          mode="standard"
          :active-speaker-id="null"
          :audio-element="null"
          voice-data-url=""
          @failed-change="live2dFailed = $event.includes(roleId)"
        />
        <div
          v-else
          class="pointer-events-none absolute h-full w-full origin-[center_0%]"
          :style="roleStyle"
        >
          <ImageAcrossFade
            v-if="imageUrl"
            class="absolute h-[102%] w-full"
            :src="imageUrl"
            :duration="0"
            position="center bottom"
            :object-fit="avatarObjectFit(size.width / size.height)"
          />
        </div>
        <div
          class="pointer-events-none absolute h-full w-full origin-[center_0%]"
          :style="roleStyle"
        >
          <button
            type="button"
            class="pointer-events-auto absolute h-[40%] w-[40%] cursor-move rounded-xl border border-dashed border-cyan-200/60 bg-contain bg-no-repeat text-left text-xl text-white/70"
            :style="{
              left: `${Number(modelValue.bubble_left || 0) + 5}%`,
              top: `${Number(modelValue.bubble_top || 0) - 5}%`,
              backgroundImage: `url(${bubbleImage})`,
            }"
            :aria-label="t('settings.roleLayout.bubble')"
            @pointerdown.stop="startDrag($event, 'bubble')"
          >
            <span class="absolute bottom-0 left-0 rounded bg-black/40 px-2 py-1">{{
              t("settings.roleLayout.bubble")
            }}</span>
          </button>
        </div>
        <div
          class="pointer-events-none absolute right-6 bottom-6 left-6 rounded-2xl border border-white/20 bg-black/35 px-6 py-4 text-2xl text-white/70"
        >
          {{ modelValue.ai_name || t("settings.roleLayout.role") }}
          <div class="mt-2 text-xl text-white/40">{{ t("settings.roleLayout.dialogue") }}</div>
        </div>
      </div>
    </div>
    <p
      v-if="(imageError && !live2dActive) || live2dFailed"
      role="status"
      class="text-xs text-amber-200/80"
    >
      {{ live2dFailed ? t("settings.roleLayout.modelFailed") : imageError }}
    </p>
    <label class="flex items-center gap-3 text-xs text-white/60">
      {{ t("settings.roleLayout.scale") }}
      <input
        type="range"
        min="0.1"
        max="3"
        step="0.01"
        :value="modelValue.scale ?? 1"
        class="min-w-0 flex-1 accent-[#79d9ff]"
        @input="patch({ scale: Number(($event.target as HTMLInputElement).value) })"
      />
      <span class="w-12 text-right tabular-nums">{{
        Number(modelValue.scale ?? 1).toFixed(2)
      }}</span>
    </label>
    <p class="text-xs text-white/40">
      {{ t("settings.roleLayout.coordinates", { width: size.width, height: size.height }) }}
    </p>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { listCharacterAvatars } from "@/api/services/character";
import { avatarObjectFit, standardAvatarStyle } from "@/utils/avatar-layout";
import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from "@/controllers/emotion/config";
import ImageAcrossFade from "@/components/ui/ImageAcrossFade.vue";
import Live2DStage from "@/components/game/live2d/Live2DStage.vue";
import type { GameRole } from "@/stores/modules/game/state";
import { prefersLive2d } from "@/types/live2d";

const props = defineProps<{
  roleId: number;
  modelValue: Record<string, any>;
  clothes?: Array<{ title: string }>;
}>();
const emit = defineEmits<{ "update:modelValue": [value: Record<string, any>] }>();
const { t } = useI18n();
const viewport = ref("landscape");
const size = computed(() =>
  viewport.value === "portrait" ? { width: 720, height: 1280 } : { width: 1280, height: 720 },
);
const frame = ref<HTMLElement>();
const frameWidth = ref(1);
const previewScale = computed(() => frameWidth.value / size.value.width);
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
const selection = ref<"role" | "bubble">("role");
const costume = ref("default");
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
const emotion = ref("正常");
const imageUrl = ref("");
const imageError = ref("");
const live2dFailed = ref(false);
let loadGeneration = 0;
watch(
  () => [props.roleId, costume.value, emotion.value],
  async () => {
    const generation = ++loadGeneration;
    imageError.value = "";
    try {
      const slots = await listCharacterAvatars(props.roleId, costume.value);
      if (generation !== loadGeneration) return;
      const slot = slots.find((s) => s.emotion === EMOTION_CONFIG_EMO[emotion.value]);
      imageUrl.value = slot?.path ? `${convertFileSrc(slot.path)}?v=${Date.now()}` : "";
      if (!imageUrl.value) imageError.value = t("settings.roleLayout.missingImage");
    } catch (e) {
      if (generation === loadGeneration) {
        imageUrl.value = "";
        imageError.value = String(e);
      }
    }
  },
  { immediate: true },
);
const previewRole = computed<GameRole>(() => ({
  roleId: props.roleId,
  roleName: props.modelValue.ai_name ?? "",
  roleSubTitle: "",
  thinkMessage: "",
  emotion: emotion.value,
  originalEmotion: emotion.value,
  show: true,
  scale: Number(props.modelValue.scale ?? 1),
  offsetX: Number(props.modelValue.offset_x ?? 0),
  offsetY: Number(props.modelValue.offset_y ?? 0),
  scaleP: 1,
  offsetXP: 0,
  offsetYP: 0,
  bubbleTop: Number(props.modelValue.bubble_top ?? 0),
  bubbleLeft: Number(props.modelValue.bubble_left ?? 0),
  clothes: {},
  clothesName: costume.value,
  bodyPart: {},
  live2d: props.modelValue.live2d,
  avatarMode: props.modelValue.avatar_mode,
  character_folder: props.modelValue.character_folder ?? "",
}));
const live2dActive = computed(
  () => prefersLive2d(previewRole.value, "standard") && !live2dFailed.value,
);
watch(
  () => [props.roleId, props.modelValue.avatar_mode, props.modelValue.live2d, costume.value],
  () => {
    live2dFailed.value = false;
  },
);
const roleStyle = computed(() => standardAvatarStyle(previewRole.value, size.value));
const bubbleImage = computed(() => {
  const image = EMOTION_CONFIG[emotion.value]?.bubbleImage;
  return new URL(
    image && image !== "none" ? image : EMOTION_CONFIG["AI思考"].bubbleImage,
    window.location.href,
  ).href;
});
function patch(value: Record<string, number>) {
  emit("update:modelValue", { ...props.modelValue, ...value });
}
function reset() {
  patch(
    selection.value === "role"
      ? { scale: 1, offset_x: 0, offset_y: 0 }
      : { bubble_left: 20, bubble_top: 5 },
  );
}
type Drag = {
  pointer: number;
  target: "role" | "bubble";
  x: number;
  y: number;
  startX: number;
  startY: number;
  scale: number;
};
let drag: Drag | undefined;
function startDrag(event: PointerEvent, target: "role" | "bubble") {
  if (event.button !== 0) return;
  selection.value = target;
  const element = event.currentTarget as HTMLElement;
  element.setPointerCapture(event.pointerId);
  element.focus({ preventScroll: true });
  drag = {
    pointer: event.pointerId,
    target,
    x: event.clientX,
    y: event.clientY,
    startX: Number(props.modelValue[target === "role" ? "offset_x" : "bubble_left"] ?? 0),
    startY: Number(props.modelValue[target === "role" ? "offset_y" : "bubble_top"] ?? 0),
    scale: previewScale.value,
  };
}
function moveDrag(event: PointerEvent) {
  if (!drag || event.pointerId !== drag.pointer) return;
  const dx = (event.clientX - drag.x) / drag.scale;
  const dy = (event.clientY - drag.y) / drag.scale;
  if (drag.target === "role")
    patch({ offset_x: Math.round(drag.startX + dx), offset_y: Math.round(drag.startY + dy) });
  else {
    const scale = Number(props.modelValue.scale) || 1;
    patch({
      bubble_left: Math.round(drag.startX + (dx / (size.value.width * scale)) * 100),
      bubble_top: Math.round(drag.startY + (dy / (size.value.height * scale)) * 100),
    });
  }
}
function finishDrag() {
  drag = undefined;
}
function zoom(event: WheelEvent) {
  patch({
    scale: Math.max(
      0.1,
      Math.min(
        3,
        Math.round(
          (Number(props.modelValue.scale ?? 1) + (event.deltaY < 0 ? 0.05 : -0.05)) * 100,
        ) / 100,
      ),
    ),
  });
}
function nudge(event: KeyboardEvent) {
  const direction = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[
    event.key
  ];
  if (!direction) return;
  event.preventDefault();
  const keys =
    selection.value === "role" ? ["offset_x", "offset_y"] : ["bubble_left", "bubble_top"];
  const step = event.shiftKey ? 10 : 1;
  patch({
    [keys[0]]: Number(props.modelValue[keys[0]] ?? 0) + direction[0] * step,
    [keys[1]]: Number(props.modelValue[keys[1]] ?? 0) + direction[1] * step,
  });
}
</script>

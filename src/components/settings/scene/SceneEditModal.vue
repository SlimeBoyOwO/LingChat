<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="show"
        class="fixed inset-0 z-9998 flex items-center justify-center bg-black/40 p-4
          backdrop-blur-md"
        @click="$emit('close')"
      >
        <div
          class="relative flex max-h-[92dvh] w-full max-w-7xl overflow-hidden rounded-3xl border
            border-white/20 bg-slate-900/40 shadow-2xl backdrop-blur-2xl"
          @click.stop
        >
          <!-- ====== 左栏：表单 ====== -->
          <div class="flex w-[420px] shrink-0 flex-col border-r border-white/10">
            <!-- Header -->
            <div
              class="flex shrink-0 items-center justify-between border-b border-white/10 bg-white/10
                p-4"
            >
              <h3 class="text-lg leading-none font-bold text-white">
                {{
                  mode === "create"
                    ? $t("settings.sceneEdit.title.create")
                    : $t("settings.sceneEdit.title.update")
                }}
              </h3>
              <button
                @click="$emit('close')"
                class="flex items-center justify-center rounded-full p-2 text-white/50
                  transition-colors hover:bg-red-500/20 hover:text-white"
              >
                <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>

            <!-- Scrollable body -->
            <div class="flex-1 space-y-4 overflow-y-auto p-4">
              <!-- 场景名称 -->
              <section>
                <label
                  class="mb-1.5 flex items-center gap-2 text-xs font-bold tracking-widest
                    text-gray-200/90 uppercase opacity-80"
                >
                  <span class="h-3 w-1 rounded-full bg-orange-500"></span>
                  {{ $t("settings.sceneEdit.label.sceneName") }}
                </label>
                <input
                  v-model="formData.sceneName"
                  :placeholder="$t('settings.sceneEdit.placeholder.sceneName')"
                  class="w-full rounded-xl border border-white/10 bg-black/40 px-3 py-2 text-sm
                    text-white placeholder-white/30 transition-all focus:border-indigo-400
                    focus:ring-1 focus:ring-indigo-400/50 focus:outline-none"
                />
              </section>

              <!-- 场景图片 -->
              <section>
                <label
                  class="mb-1.5 flex items-center gap-2 text-xs font-bold tracking-widest
                    text-gray-200/90 uppercase opacity-80"
                >
                  <span class="h-3 w-1 rounded-full bg-indigo-500"></span>
                  {{ $t("settings.sceneEdit.label.sceneImage") }}
                </label>
                <div class="flex gap-2">
                  <select
                    v-model="formData.sceneImage"
                    class="flex-1 appearance-none rounded-xl border border-white/10 bg-black/40 px-3
                      py-2 text-sm text-white transition-all focus:border-indigo-400 focus:ring-1
                      focus:ring-indigo-400/50 focus:outline-none"
                  >
                    <option value="" class="bg-slate-800 text-white/50">
                      {{ $t("settings.sceneEdit.option.selectBackground") }}
                    </option>
                    <option
                      v-for="bg in backgrounds"
                      :key="bg.url"
                      :value="bg.url"
                      class="bg-slate-800 text-white"
                    >
                      {{ bg.title }}
                    </option>
                  </select>
                  <Button
                    @click="$emit('upload')"
                    class="rounded-xl border border-white/20 bg-white/10! px-4 text-sm!
                      whitespace-nowrap text-white! hover:bg-white/20!"
                  >
                    {{ $t("settings.sceneEdit.button.upload") }}
                  </Button>
                </div>
              </section>

              <!-- 场景描述 -->
              <section>
                <label
                  class="mb-1.5 flex items-center gap-2 text-xs font-bold tracking-widest
                    text-gray-200/90 uppercase opacity-80"
                >
                  <span class="h-3 w-1 rounded-full bg-emerald-500"></span>
                  {{ $t("settings.sceneEdit.label.sceneDescription") }}
                </label>
                <textarea
                  v-model="formData.sceneDescription"
                  :placeholder="$t('settings.sceneEdit.placeholder.sceneDescription')"
                  rows="2"
                  class="w-full resize-none rounded-xl border border-white/10 bg-black/40 px-3 py-2
                    text-sm text-white placeholder-white/30 transition-all focus:border-indigo-400
                    focus:ring-1 focus:ring-indigo-400/50 focus:outline-none"
                ></textarea>
              </section>

              <!-- 光影参数（可折叠） -->
              <section>
                <button
                  @click="showLighting = !showLighting"
                  class="mb-1.5 flex w-full items-center justify-between gap-2 text-xs font-bold
                    tracking-widest text-gray-200/90 uppercase opacity-80 transition-opacity
                    hover:opacity-100"
                >
                  <span class="flex items-center gap-2">
                    <span class="h-3 w-1 rounded-full bg-purple-500"></span>
                    {{ $t("settings.sceneEdit.lighting.title") }}
                  </span>
                  <svg
                    :class="['h-3.5 w-3.5 transition-transform', showLighting ? 'rotate-90' : '']"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M9 5l7 7-7 7"
                    />
                  </svg>
                </button>

                <div v-show="showLighting" class="space-y-2.5 pl-3">
                  <label class="flex cursor-pointer items-center gap-2">
                    <input
                      v-model="formData.lightingEnabled"
                      type="checkbox"
                      class="h-3.5 w-3.5 rounded accent-purple-500"
                    />
                    <span class="text-xs text-white/70">{{
                      $t("settings.sceneEdit.lighting.enableForScene")
                    }}</span>
                  </label>

                  <template v-if="formData.lightingEnabled">
                    <label class="flex cursor-pointer items-center gap-2 pl-4">
                      <input
                        v-model="formData.overlayEnabled"
                        type="checkbox"
                        class="h-3.5 w-3.5 rounded accent-amber-500"
                      />
                      <span class="text-xs text-white/70">{{
                        $t("settings.sceneEdit.lighting.enableOverlay")
                      }}</span>
                    </label>

                    <!-- ======== 角色滤镜 ======== -->
                    <div class="pt-1 text-[11px] font-bold tracking-wider text-white/40 uppercase">
                      {{ $t("settings.sceneEdit.filter.character") }}
                    </div>

                    <div
                      v-for="s in charFilterSliders"
                      :key="'char-' + s.key"
                      class="flex items-center gap-2"
                    >
                      <span class="w-16 shrink-0 text-[11px] text-white/50">{{ s.label }}</span>
                      <input
                        type="range"
                        :min="s.min"
                        :max="s.max"
                        :step="s.step"
                        v-model.number="formData[s.key]"
                        class="lighting-range flex-1"
                        :style="{ '--accent-color': s.color }"
                      />
                      <span class="w-12 text-right text-[11px] text-white/50 tabular-nums"
                        >{{ formData[s.key] }}{{ s.unit }}</span
                      >
                    </div>

                    <div class="flex items-center gap-3">
                      <span class="shrink-0 text-[11px] text-white/50">{{
                        $t("settings.sceneEdit.label.glowColor")
                      }}</span>
                      <input
                        v-model="formData.charGlowColor"
                        type="color"
                        class="h-6 w-6 shrink-0 cursor-pointer rounded border-0 bg-transparent"
                      />
                      <span class="truncate text-[11px] text-white/30">{{
                        formData.charGlowColor
                      }}</span>
                    </div>

                    <!-- ======== 背景滤镜 ======== -->
                    <div class="pt-1 text-[11px] font-bold tracking-wider text-white/40 uppercase">
                      {{ $t("settings.sceneEdit.filter.background") }}
                    </div>

                    <div
                      v-for="s in bgFilterSliders"
                      :key="'bg-' + s.key"
                      class="flex items-center gap-2"
                    >
                      <span class="w-16 shrink-0 text-[11px] text-white/50">{{ s.label }}</span>
                      <input
                        type="range"
                        :min="s.min"
                        :max="s.max"
                        :step="s.step"
                        v-model.number="formData[s.key]"
                        class="lighting-range flex-1"
                        :style="{ '--accent-color': s.color }"
                      />
                      <span class="w-12 text-right text-[11px] text-white/50 tabular-nums"
                        >{{ formData[s.key] }}{{ s.unit }}</span
                      >
                    </div>

                    <div class="flex items-center gap-3">
                      <span class="shrink-0 text-[11px] text-white/50">{{
                        $t("settings.sceneEdit.label.glowColor")
                      }}</span>
                      <input
                        v-model="formData.bgGlowColor"
                        type="color"
                        class="h-6 w-6 shrink-0 cursor-pointer rounded border-0 bg-transparent"
                      />
                      <span class="truncate text-[11px] text-white/30">{{
                        formData.bgGlowColor
                      }}</span>
                    </div>

                    <!-- ======== 光照叠加（仅 overlayEnabled 时显示） ======== -->
                    <template v-if="formData.overlayEnabled">
                      <div
                        class="pt-1 text-[11px] font-bold tracking-wider text-white/40 uppercase"
                      >
                        {{ $t("settings.sceneEdit.overlay.title") }}
                      </div>

                      <div class="flex items-center gap-3">
                        <span class="shrink-0 text-[11px] text-white/50">{{
                          $t("settings.sceneEdit.overlay.blend")
                        }}</span>
                        <select
                          v-model="formData.blendMode"
                          class="flex-1 rounded-lg border border-white/10 bg-black/40 px-2 py-1
                            text-[11px] text-white focus:ring-1 focus:ring-purple-400/50
                            focus:outline-none"
                        >
                          <option value="normal">Normal</option>
                          <option value="multiply">Multiply</option>
                          <option value="screen">Screen</option>
                          <option value="overlay">Overlay</option>
                          <option value="soft-light">Soft Light</option>
                          <option value="hard-light">Hard Light</option>
                          <option value="color-dodge">Color Dodge</option>
                          <option value="color-burn">Color Burn</option>
                          <option value="difference">Difference</option>
                        </select>
                      </div>

                      <div
                        v-for="s in overlaySliders"
                        :key="'ov-' + s.key"
                        class="flex items-center gap-2"
                      >
                        <span class="w-16 shrink-0 text-[11px] text-white/50">{{ s.label }}</span>
                        <input
                          type="range"
                          :min="s.min"
                          :max="s.max"
                          :step="s.step"
                          v-model.number="formData[s.key]"
                          class="lighting-range flex-1"
                          :style="{ '--accent-color': s.color }"
                        />
                        <span class="w-12 text-right text-[11px] text-white/50 tabular-nums"
                          >{{ formData[s.key] }}{{ s.unit }}</span
                        >
                      </div>

                      <div class="flex items-center gap-3">
                        <span class="shrink-0 text-[11px] text-white/50">{{
                          $t("settings.sceneEdit.overlay.centerColor")
                        }}</span>
                        <input
                          v-model="formData.overlayColor1"
                          type="color"
                          class="h-6 w-6 shrink-0 cursor-pointer rounded border-0 bg-transparent"
                        />
                        <span class="truncate text-[11px] text-white/30">{{
                          formData.overlayColor1
                        }}</span>
                      </div>
                      <div class="flex items-center gap-3">
                        <span class="shrink-0 text-[11px] text-white/50">{{
                          $t("settings.sceneEdit.overlay.edgeColor")
                        }}</span>
                        <input
                          v-model="formData.overlayColor2"
                          type="color"
                          class="h-6 w-6 shrink-0 cursor-pointer rounded border-0 bg-transparent"
                        />
                        <span class="truncate text-[11px] text-white/30">{{
                          formData.overlayColor2
                        }}</span>
                      </div>

                      <div
                        v-for="s in overlayExtraSliders"
                        :key="'ove-' + s.key"
                        class="flex items-center gap-2"
                      >
                        <span class="w-16 shrink-0 text-[11px] text-white/50">{{ s.label }}</span>
                        <input
                          type="range"
                          :min="s.min"
                          :max="s.max"
                          :step="s.step"
                          v-model.number="formData[s.key]"
                          class="lighting-range flex-1"
                          :style="{ '--accent-color': s.color }"
                        />
                        <span class="w-12 text-right text-[11px] text-white/50 tabular-nums"
                          >{{ formData[s.key] }}{{ s.unit }}</span
                        >
                      </div>

                      <div class="flex items-center gap-3">
                        <span class="shrink-0 text-[11px] text-white/50">{{
                          $t("settings.sceneEdit.overlay.target")
                        }}</span>
                        <select
                          v-model="formData.overlayTarget"
                          class="flex-1 rounded-lg border border-white/10 bg-black/40 px-2 py-1
                            text-[11px] text-white focus:ring-1 focus:ring-purple-400/50
                            focus:outline-none"
                        >
                          <option value="both">
                            {{ $t("settings.sceneEdit.overlayTarget.both") }}
                          </option>
                          <option value="character">
                            {{ $t("settings.sceneEdit.overlayTarget.character") }}
                          </option>
                          <option value="background">
                            {{ $t("settings.sceneEdit.overlayTarget.background") }}
                          </option>
                        </select>
                      </div>
                    </template>

                    <button
                      @click="resetLighting"
                      class="text-[11px] text-white/40 underline transition-colors
                        hover:text-white/80"
                    >
                      {{ $t("settings.sceneEdit.button.resetDefault") }}
                    </button>
                  </template>
                </div>
              </section>
            </div>

            <!-- Footer -->
            <div
              class="flex shrink-0 items-center justify-end gap-2 border-t border-white/10
                bg-white/5 p-3"
            >
              <Button
                @click="$emit('close')"
                class="border border-white/20 bg-transparent! text-sm! text-white/70!
                  hover:bg-white/10! hover:text-white!"
              >
                {{ $t("settings.sceneEdit.button.cancel") }}
              </Button>
              <Button
                @click="handleSubmit"
                :disabled="!formData.sceneName.trim()"
                class="min-w-[70px] border-none bg-indigo-500! text-sm! text-white
                  shadow-[0_0_10px_rgba(99,102,241,0.5)] hover:bg-indigo-400! disabled:opacity-50
                  disabled:shadow-none"
              >
                {{
                  mode === "create"
                    ? $t("settings.sceneEdit.button.create")
                    : $t("settings.sceneEdit.button.update")
                }}
              </Button>
            </div>
          </div>

          <!-- ====== 右栏：实时预览 ====== -->
          <div class="flex min-w-0 flex-1 flex-col bg-black/20 p-5">
            <span class="mb-3 shrink-0 text-xs font-bold tracking-widest text-white/40 uppercase">{{
              $t("settings.sceneEdit.preview.title")
            }}</span>
            <div
              class="relative flex min-h-0 flex-1 items-center justify-center overflow-hidden
                rounded-xl border border-white/10 bg-slate-800/50"
            >
              <!-- 背景图（应用背景滤镜） -->
              <img
                v-if="selectedBackgroundPreview"
                :src="selectedBackgroundPreview"
                class="absolute inset-0 h-full w-full object-cover"
                :style="previewBgFilterStyle"
              />
              <!-- 角色立绘（应用角色滤镜 + 位置/缩放） -->
              <img
                v-if="previewAvatarUrl"
                :src="previewAvatarUrl"
                class="absolute"
                :style="previewCharStyle"
              />
              <!-- 光照叠加层 -->
              <div
                v-if="formData.lightingEnabled && formData.overlayEnabled"
                class="pointer-events-none absolute inset-0 z-10"
                :style="previewOverlayStyle"
              ></div>
              <!-- 占位 -->
              <div v-if="!selectedBackgroundPreview" class="text-sm text-white/20">
                {{ $t("settings.sceneEdit.preview.placeholder") }}
              </div>
            </div>
            <!-- 信息标签 -->
            <div class="mt-2 flex shrink-0 gap-3 text-xs text-white/40">
              <span
                v-if="
                  formData.lightingEnabled &&
                  formData.overlayEnabled &&
                  formData.blendMode !== 'normal'
                "
                >{{
                  $t("settings.sceneEdit.preview.blendMode", { mode: formData.blendMode })
                }}</span
              >
              <span v-if="previewAvatarUrl">{{
                $t("settings.sceneEdit.preview.avatarLoaded")
              }}</span>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
  import { computed, reactive, ref, watch } from "vue";
  import { useI18n } from "vue-i18n";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { Button } from "../../base";
  import { useGameStore } from "../../../stores/modules/game";
  import { EMOTION_CONFIG_EMO } from "../../../controllers/emotion/config";
  import type { BackgroundImageInfo } from "../../../types";
  import type { LightingParams, FilterParams } from "../../../api/services/scene";

  const props = defineProps<{
    show: boolean;
    mode: "create" | "update";
    backgrounds: BackgroundImageInfo[];
    initialData?: {
      sceneName: string;
      sceneImage: string | null;
      sceneDescription: string;
      lighting?: LightingParams | null;
    };
  }>();

  const emit = defineEmits<{
    close: [];
    submit: [
      data: {
        sceneName: string;
        sceneImage: string | null;
        sceneDescription: string;
        lighting?: LightingParams | null;
      },
    ];
    upload: [];
  }>();

  const gameStore = useGameStore();
  const { t } = useI18n();

  // ---- defaults ----

  const DEF = {
    brightness: 1.0,
    contrast: 1.0,
    saturation: 1.0,
    sepia: 0.0,
    glowRadius: 0,
    glowColor: "#ffaa33",
    blendMode: "normal",
    lightX: 50,
    lightY: 50,
    overlayColor1: "#ffb44b",
    overlayColor2: "#18202e",
    overlayRadius: 80,
    overlayOpacity: 50, // slider 0–100, saved as 0.0–1.0
    overlayTarget: "both",
  };

  // ---- reactive form ----

  // 使用 interface 保持类型安全
  interface FormData {
    sceneName: string;
    sceneImage: string;
    sceneDescription: string;
    lightingEnabled: boolean;
    overlayEnabled: boolean;
    // 角色滤镜
    charBrightness: number;
    charContrast: number;
    charSaturation: number;
    charSepia: number;
    charGlowRadius: number;
    charGlowColor: string;
    // 背景滤镜
    bgBrightness: number;
    bgContrast: number;
    bgSaturation: number;
    bgSepia: number;
    bgGlowRadius: number;
    bgGlowColor: string;
    // 叠加
    blendMode: string;
    lightX: number;
    lightY: number;
    overlayColor1: string;
    overlayColor2: string;
    overlayRadius: number;
    overlayOpacity: number;
    overlayTarget: string;
    [key: string]: string | number | boolean; // 索引签名以支持 v-model="formData[key]"
  }

  const formData = reactive<FormData>({
    sceneName: "",
    sceneImage: "",
    sceneDescription: "",
    lightingEnabled: false,
    overlayEnabled: false,
    charBrightness: DEF.brightness,
    charContrast: DEF.contrast,
    charSaturation: DEF.saturation,
    charSepia: DEF.sepia,
    charGlowRadius: DEF.glowRadius,
    charGlowColor: DEF.glowColor,
    bgBrightness: DEF.brightness,
    bgContrast: DEF.contrast,
    bgSaturation: DEF.saturation,
    bgSepia: DEF.sepia,
    bgGlowRadius: DEF.glowRadius,
    bgGlowColor: DEF.glowColor,
    blendMode: DEF.blendMode,
    lightX: DEF.lightX,
    lightY: DEF.lightY,
    overlayColor1: DEF.overlayColor1,
    overlayColor2: DEF.overlayColor2,
    overlayRadius: DEF.overlayRadius,
    overlayOpacity: DEF.overlayOpacity,
    overlayTarget: DEF.overlayTarget,
  });

  // ---- slider configs ----

  interface SliderDef {
    key: keyof FormData;
    label: string;
    min: number;
    max: number;
    step: number;
    color: string;
    unit: string;
  }

  const charFilterSliders = computed<SliderDef[]>(() => [
    {
      key: "charBrightness",
      label: t("settings.sceneEdit.slider.brightness"),
      min: 0.3,
      max: 2.2,
      step: 0.01,
      color: "#8b5cf6",
      unit: "",
    },
    {
      key: "charContrast",
      label: t("settings.sceneEdit.slider.contrast"),
      min: 0.5,
      max: 2.0,
      step: 0.01,
      color: "#8b5cf6",
      unit: "",
    },
    {
      key: "charSaturation",
      label: t("settings.sceneEdit.slider.saturation"),
      min: 0.0,
      max: 2.5,
      step: 0.01,
      color: "#8b5cf6",
      unit: "",
    },
    {
      key: "charSepia",
      label: t("settings.sceneEdit.slider.sepia"),
      min: 0.0,
      max: 1.0,
      step: 0.01,
      color: "#8b5cf6",
      unit: "",
    },
    {
      key: "charGlowRadius",
      label: t("settings.sceneEdit.slider.glowRadius"),
      min: 0,
      max: 50,
      step: 1,
      color: "#f59e0b",
      unit: "px",
    },
  ]);

  const bgFilterSliders = computed<SliderDef[]>(() => [
    {
      key: "bgBrightness",
      label: t("settings.sceneEdit.slider.brightness"),
      min: 0.3,
      max: 2.2,
      step: 0.01,
      color: "#06b6d4",
      unit: "",
    },
    {
      key: "bgContrast",
      label: t("settings.sceneEdit.slider.contrast"),
      min: 0.5,
      max: 2.0,
      step: 0.01,
      color: "#06b6d4",
      unit: "",
    },
    {
      key: "bgSaturation",
      label: t("settings.sceneEdit.slider.saturation"),
      min: 0.0,
      max: 2.5,
      step: 0.01,
      color: "#06b6d4",
      unit: "",
    },
    {
      key: "bgSepia",
      label: t("settings.sceneEdit.slider.sepia"),
      min: 0.0,
      max: 1.0,
      step: 0.01,
      color: "#06b6d4",
      unit: "",
    },
    {
      key: "bgGlowRadius",
      label: t("settings.sceneEdit.slider.glowRadius"),
      min: 0,
      max: 50,
      step: 1,
      color: "#f59e0b",
      unit: "px",
    },
  ]);

  const overlaySliders = computed<SliderDef[]>(() => [
    {
      key: "lightX",
      label: t("settings.sceneEdit.slider.lightX"),
      min: 0,
      max: 100,
      step: 1,
      color: "#fbbf24",
      unit: "%",
    },
    {
      key: "lightY",
      label: t("settings.sceneEdit.slider.lightY"),
      min: 0,
      max: 100,
      step: 1,
      color: "#fbbf24",
      unit: "%",
    },
  ]);

  const overlayExtraSliders = computed<SliderDef[]>(() => [
    {
      key: "overlayRadius",
      label: t("settings.sceneEdit.slider.overlayRadius"),
      min: 10,
      max: 100,
      step: 1,
      color: "#fbbf24",
      unit: "%",
    },
    {
      key: "overlayOpacity",
      label: t("settings.sceneEdit.slider.overlayOpacity"),
      min: 0,
      max: 100,
      step: 1,
      color: "#fbbf24",
      unit: "%",
    },
  ]);

  // ---- sub-state ----

  const showLighting = ref(false);
  const previewAvatarUrl = ref("");

  // ---- reset ----

  function resetLighting() {
    formData.lightingEnabled = true;
    formData.overlayEnabled = false;
    formData.charBrightness = DEF.brightness;
    formData.charContrast = DEF.contrast;
    formData.charSaturation = DEF.saturation;
    formData.charSepia = DEF.sepia;
    formData.charGlowRadius = DEF.glowRadius;
    formData.charGlowColor = DEF.glowColor;
    formData.bgBrightness = DEF.brightness;
    formData.bgContrast = DEF.contrast;
    formData.bgSaturation = DEF.saturation;
    formData.bgSepia = DEF.sepia;
    formData.bgGlowRadius = DEF.glowRadius;
    formData.bgGlowColor = DEF.glowColor;
    formData.blendMode = DEF.blendMode;
    formData.lightX = DEF.lightX;
    formData.lightY = DEF.lightY;
    formData.overlayColor1 = DEF.overlayColor1;
    formData.overlayColor2 = DEF.overlayColor2;
    formData.overlayRadius = DEF.overlayRadius;
    formData.overlayOpacity = DEF.overlayOpacity;
    formData.overlayTarget = DEF.overlayTarget;
  }

  // ---- role avatar ----

  async function resolveAvatar() {
    previewAvatarUrl.value = "";
    const roleId = gameStore.currentInteractRoleId;
    if (!roleId) return;
    const role = gameStore.gameRoles[roleId];
    if (!role) return;
    const clothesName =
      role.clothesName === "默认" || !role.clothesName ? "default" : role.clothesName;
    const mappedEmotion = EMOTION_CONFIG_EMO[role.emotion] || "正常";
    try {
      const path: string = await invoke("get_avatar_file", {
        characterFolder: role.character_folder,
        emotion: mappedEmotion,
        clothesName,
      });
      previewAvatarUrl.value = convertFileSrc(path);
    } catch {
      previewAvatarUrl.value = "";
    }
  }

  // ---- init from initialData ----

  watch(
    () => props.show,
    (val) => {
      if (val && props.initialData) {
        formData.sceneName = props.initialData.sceneName;
        formData.sceneImage = props.initialData.sceneImage || "";
        formData.sceneDescription = props.initialData.sceneDescription;
        const l = props.initialData.lighting;
        if (l) {
          formData.lightingEnabled = true;
          formData.overlayEnabled = l.overlay_enabled;
          formData.charBrightness = l.character.brightness;
          formData.charContrast = l.character.contrast;
          formData.charSaturation = l.character.saturation;
          formData.charSepia = l.character.sepia;
          formData.charGlowRadius = l.character.glow_radius;
          formData.charGlowColor = l.character.glow_color;
          formData.bgBrightness = l.background.brightness;
          formData.bgContrast = l.background.contrast;
          formData.bgSaturation = l.background.saturation;
          formData.bgSepia = l.background.sepia;
          formData.bgGlowRadius = l.background.glow_radius;
          formData.bgGlowColor = l.background.glow_color;
          formData.blendMode = l.blend_mode;
          formData.lightX = l.light_x;
          formData.lightY = l.light_y;
          formData.overlayColor1 = l.overlay_color1;
          formData.overlayColor2 = l.overlay_color2;
          formData.overlayRadius = l.overlay_radius ?? 80;
          formData.overlayOpacity = (l.overlay_opacity ?? 0.5) * 100;
          formData.overlayTarget = l.overlay_target || "both";
          showLighting.value = true;
        } else {
          formData.lightingEnabled = false;
          resetLighting();
          showLighting.value = false;
        }
        resolveAvatar();
      } else if (val) {
        formData.sceneName = "";
        formData.sceneImage = "";
        formData.sceneDescription = "";
        formData.lightingEnabled = false;
        resetLighting();
        showLighting.value = false;
        resolveAvatar();
      }
    }
  );

  // ---- preview computed ----

  const selectedBackgroundPreview = computed(() => {
    if (!formData.sceneImage) return null;
    return convertFileSrc(formData.sceneImage);
  });

  function buildFilterString(
    brightness: number,
    contrast: number,
    saturation: number,
    sepia: number,
    glowRadius: number,
    glowColor: string
  ) {
    const parts: string[] = [];
    if (brightness !== 1.0) parts.push(`brightness(${brightness})`);
    if (contrast !== 1.0) parts.push(`contrast(${contrast})`);
    if (saturation !== 1.0) parts.push(`saturate(${saturation})`);
    if (sepia > 0) parts.push(`sepia(${sepia})`);
    if (glowRadius > 0) parts.push(`drop-shadow(0 0 ${glowRadius}px ${glowColor})`);
    return parts.join(" ");
  }

  const previewBgFilterStyle = computed(() => {
    if (!formData.lightingEnabled) return undefined;
    const filter = buildFilterString(
      formData.bgBrightness,
      formData.bgContrast,
      formData.bgSaturation,
      formData.bgSepia,
      formData.bgGlowRadius,
      formData.bgGlowColor
    );
    return filter ? { filter } : undefined;
  });

  const previewCharFilter = computed(() => {
    if (!formData.lightingEnabled) return undefined;
    return buildFilterString(
      formData.charBrightness,
      formData.charContrast,
      formData.charSaturation,
      formData.charSepia,
      formData.charGlowRadius,
      formData.charGlowColor
    );
  });

  const previewCharPosition = computed(() => {
    const roleId = gameStore.currentInteractRoleId;
    if (!roleId) return {};
    const role = gameStore.gameRoles[roleId];
    if (!role) return {};
    return {
      left: `calc(50% + ${role.offsetX || 0}px)`,
      top: `${role.offsetY || 0}px`,
      transform: `translateX(-50%) scale(${role.scale || 1})`,
      transformOrigin: "center 0%",
    };
  });

  const previewCharStyle = computed(() => {
    const pos = previewCharPosition.value;
    const filter = previewCharFilter.value;
    const style: Record<string, string> = {
      left: pos.left || "50%",
      top: pos.top || "0px",
      transform: pos.transform || "translateX(-50%) scale(1)",
      transformOrigin: pos.transformOrigin || "center 0%",
      maxHeight: "100%",
      maxWidth: "100%",
    };
    if (filter) {
      style.filter = filter;
    }
    return style;
  });

  const previewOverlayStyle = computed(() => {
    if (!formData.lightingEnabled || !formData.overlayEnabled) return undefined;
    const blend = formData.blendMode !== "normal" ? formData.blendMode : "overlay";
    return `background: radial-gradient(circle at ${formData.lightX}% ${formData.lightY}%, ${formData.overlayColor1} 0%, ${formData.overlayColor2} ${formData.overlayRadius}%); mix-blend-mode: ${blend}; opacity: ${formData.overlayOpacity / 100}`;
  });

  // ---- submit ----

  const handleSubmit = () => {
    let lighting: LightingParams | null = null;
    if (formData.lightingEnabled) {
      const charFilter: FilterParams = {
        brightness: formData.charBrightness,
        contrast: formData.charContrast,
        saturation: formData.charSaturation,
        sepia: formData.charSepia,
        glow_radius: formData.charGlowRadius,
        glow_color: formData.charGlowColor,
      };
      const bgFilter: FilterParams = {
        brightness: formData.bgBrightness,
        contrast: formData.bgContrast,
        saturation: formData.bgSaturation,
        sepia: formData.bgSepia,
        glow_radius: formData.bgGlowRadius,
        glow_color: formData.bgGlowColor,
      };
      lighting = {
        character: charFilter,
        background: bgFilter,
        overlay_enabled: formData.overlayEnabled,
        blend_mode: formData.blendMode,
        light_x: formData.lightX,
        light_y: formData.lightY,
        overlay_color1: formData.overlayColor1,
        overlay_color2: formData.overlayColor2,
        overlay_radius: formData.overlayRadius,
        overlay_opacity: formData.overlayOpacity / 100,
        overlay_target: formData.overlayTarget,
      };
    }
    emit("submit", {
      sceneName: formData.sceneName.trim(),
      sceneImage: formData.sceneImage || null,
      sceneDescription: formData.sceneDescription.trim(),
      lighting,
    });
  };
</script>

<style scoped>
  /* 实时响应的 range input 样式 */
  .lighting-range {
    -webkit-appearance: none;
    appearance: none;
    height: 6px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.15);
    outline: none;
    cursor: pointer;
  }

  .lighting-range::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--accent-color, #8b5cf6);
    border: 2px solid rgba(255, 255, 255, 0.8);
    cursor: grab;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .lighting-range::-webkit-slider-thumb:active {
    cursor: grabbing;
    transform: scale(1.15);
  }
</style>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="show"
        class="fixed inset-0 z-9999 flex items-center justify-center bg-black/50 p-4
          backdrop-blur-md"
        @click="close"
      >
        <div
          class="relative flex max-h-[92dvh] w-full max-w-5xl flex-col overflow-hidden rounded-3xl
            border border-white/20 bg-slate-900/85 shadow-2xl backdrop-blur-2xl md:flex-row"
          @click.stop
        >
          <!-- ============ 左栏：调参 ============ -->
          <div
            class="flex min-w-0 flex-1 flex-col border-r border-white/10 md:w-[440px] md:shrink-0"
          >
            <div
              class="flex shrink-0 items-center justify-between border-b border-white/10 bg-white/10
                p-4"
            >
              <h3 class="text-base leading-none font-bold text-white">
                {{
                  editing
                    ? $t("settings.background.lighting.editor.titleEdit")
                    : $t("settings.background.lighting.editor.titleNew")
                }}
              </h3>
              <button
                @click="close"
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

            <div class="flex-1 space-y-4 overflow-y-auto p-4">
              <!-- 名称 / 描述 / 心情 -->
              <div class="space-y-2">
                <input
                  v-model="name"
                  :placeholder="$t('settings.background.lighting.editor.namePlaceholder')"
                  maxlength="24"
                  class="w-full rounded-xl border border-white/10 bg-black/40 px-3 py-2 text-sm
                    text-white placeholder-white/30 focus:border-amber-400/60 focus:ring-1
                    focus:ring-amber-400/40 focus:outline-none"
                />
                <input
                  v-model="description"
                  :placeholder="$t('settings.background.lighting.editor.descriptionPlaceholder')"
                  maxlength="60"
                  class="w-full rounded-xl border border-white/10 bg-black/40 px-2.5 py-1.5 text-xs
                    text-white placeholder-white/30 focus:border-amber-400/60 focus:outline-none"
                />
                <input
                  v-model="mood"
                  :placeholder="$t('settings.background.lighting.editor.moodPlaceholder')"
                  class="w-full rounded-xl border border-white/10 bg-black/40 px-2.5 py-1.5 text-xs
                    text-white placeholder-white/30 focus:border-amber-400/60 focus:outline-none"
                />
                <div class="text-[11px] leading-snug text-white/35">
                  {{ $t("settings.background.lighting.editor.moodHint") }}
                </div>
              </div>

              <!-- 各个光影层 -->
              <section
                v-for="sec in sections"
                :key="sec.key"
                class="rounded-xl border border-white/10 bg-white/5 p-3"
              >
                <label class="flex cursor-pointer items-center gap-2">
                  <input
                    v-if="sec.enable"
                    type="checkbox"
                    class="h-3.5 w-3.5 rounded accent-amber-500"
                    :checked="readBool(sec.enable)"
                    @change="writeBool(sec.enable, ($event.target as HTMLInputElement).checked)"
                  />
                  <span class="text-xs font-bold tracking-wider text-white/80">{{
                    sec.title
                  }}</span>
                </label>
                <div v-if="sec.tip" class="mt-1 text-[11px] leading-snug text-white/35">
                  {{ sec.tip }}
                </div>

                <div
                  class="mt-2 space-y-2"
                  :class="{ 'pointer-events-none opacity-35': sec.enable && !readBool(sec.enable) }"
                >
                  <!-- 灯位拖拽板 -->
                  <div
                    v-if="sec.pad"
                    class="relative h-28 w-full cursor-crosshair touch-none overflow-hidden
                      rounded-lg border border-white/15 select-none"
                    :style="padStyle"
                    @pointerdown="startDrag"
                    @pointermove="onDrag"
                    @pointerup="endDrag"
                    @pointercancel="endDrag"
                  >
                    <div
                      class="pointer-events-none absolute h-5 w-5 -translate-x-1/2 -translate-y-1/2
                        rounded-full border-2 border-white bg-amber-300
                        shadow-[0_0_14px_4px_rgba(252,211,77,0.8)]"
                      :style="{ left: `${draft.light_x}%`, top: `${draft.light_y}%` }"
                    ></div>
                  </div>
                  <div v-if="sec.pad" class="flex items-center gap-3 text-[11px] text-white/45">
                    <span>{{ $t("settings.background.lighting.editor.angle") }}</span>
                    <span class="font-bold text-amber-300/90 tabular-nums"
                      >{{ Math.round(lightAngle) }}°</span
                    >
                    <span class="text-white/25">·</span>
                    <span class="tabular-nums">{{ draft.light_x }}% / {{ draft.light_y }}%</span>
                  </div>

                  <div v-for="sel in sec.selects" :key="sel.path" class="flex items-center gap-2">
                    <span class="w-[5.5rem] shrink-0 text-[11px] text-white/50">{{
                      sel.label
                    }}</span>
                    <select
                      class="min-w-0 flex-1 rounded-lg border border-white/10 bg-black/40 px-2 py-1
                        text-[11px] text-white focus:border-amber-400/60 focus:outline-none"
                      :value="readStr(sel.path)"
                      @change="writeStr(sel.path, ($event.target as HTMLSelectElement).value)"
                    >
                      <option v-for="o in sel.options" :key="o.value" :value="o.value">
                        {{ o.label }}
                      </option>
                    </select>
                  </div>

                  <template v-for="(grp, gi) in sec.groups" :key="gi">
                    <div
                      v-if="grp.title"
                      class="pt-1 text-[11px] font-bold tracking-wider text-white/40 uppercase"
                    >
                      {{ grp.title }}
                    </div>
                    <LightingSliderRow
                      v-for="r in grp.rows"
                      :key="r.path"
                      :label="r.label"
                      :min="r.min"
                      :max="r.max"
                      :step="r.step"
                      :unit="r.unit"
                      :decimals="r.decimals"
                      :color="r.color"
                      :model-value="readNum(r)"
                      @update:model-value="writeNum(r, $event)"
                    />
                    <LightingColorRow
                      v-for="c in grp.colors"
                      :key="c.path"
                      :label="c.label"
                      :model-value="readStr(c.path)"
                      @update:model-value="writeStr(c.path, $event)"
                    />
                  </template>
                </div>
              </section>

              <button
                @click="resetAll"
                class="text-[11px] text-white/40 underline transition-colors hover:text-white/80"
              >
                {{ $t("settings.background.lighting.editor.reset") }}
              </button>
            </div>

            <div
              class="flex shrink-0 items-center justify-between gap-2 border-t border-white/10
                bg-white/5 p-3"
            >
              <button
                @click="revert"
                class="text-[11px] text-white/40 underline transition-colors hover:text-white/80"
              >
                {{ $t("settings.background.lighting.editor.revert") }}
              </button>
              <div class="flex items-center gap-2">
                <Button
                  @click="close"
                  class="border border-white/20 bg-transparent! text-sm! text-white/70!
                    hover:bg-white/10! hover:text-white!"
                >
                  {{ $t("settings.background.lighting.editor.cancel") }}
                </Button>
                <Button
                  @click="save"
                  :disabled="!name.trim() || saving"
                  class="min-w-[70px] border-none bg-amber-500! text-sm! text-white
                    shadow-[0_0_10px_rgba(245,158,11,0.5)] hover:bg-amber-400! disabled:opacity-50
                    disabled:shadow-none"
                >
                  {{
                    saving
                      ? $t("settings.background.lighting.editor.saving")
                      : $t("settings.background.lighting.editor.save")
                  }}
                </Button>
              </div>
            </div>
          </div>

          <!-- ============ 右栏：实时预览 ============ -->
          <div class="flex min-w-0 flex-1 flex-col bg-black/30 p-5">
            <span class="mb-1 shrink-0 text-xs font-bold tracking-widest text-white/40 uppercase">
              {{ $t("settings.background.lighting.editor.previewTitle") }}
            </span>
            <div class="mb-3 shrink-0 text-[11px] leading-snug text-white/35">
              {{ $t("settings.background.lighting.editor.previewHint") }}
            </div>
            <div
              class="relative min-h-[16rem] flex-1 overflow-hidden rounded-xl border border-white/10
                bg-slate-800"
              style="isolation: isolate"
            >
              <!-- 混合层要有一张不透明的底才能算出「打光」；没有场景图时透明底会让
                   screen / soft-light 直接变成一层色块，预览和真实画面完全对不上。 -->
              <div class="absolute inset-0 bg-slate-800"></div>
              <img
                v-if="bgSrc"
                :src="bgSrc"
                class="absolute inset-0 h-full w-full object-cover"
                :style="
                  previewPlan.backgroundFilter ? { filter: previewPlan.backgroundFilter } : {}
                "
                alt=""
              />
              <div
                v-if="previewPlan.bloom && bgSrc"
                class="pointer-events-none absolute inset-0"
                :class="previewPlan.bloom.className"
                :style="previewPlan.bloom.style"
              >
                <img :src="bgSrc" class="h-full w-full object-cover" alt="" />
              </div>
              <div
                v-if="previewPlan.bgOverlay"
                class="pointer-events-none absolute inset-0"
                :style="previewPlan.bgOverlay"
              ></div>
              <img
                v-if="avatarSrc"
                :src="avatarSrc"
                class="absolute bottom-0 left-1/2 h-[88%] -translate-x-1/2 object-contain"
                :style="previewPlan.characterFilter ? { filter: previewPlan.characterFilter } : {}"
                alt=""
              />
              <div
                v-if="previewPlan.stageOverlay"
                class="pointer-events-none absolute inset-0"
                :style="previewPlan.stageOverlay"
              ></div>
              <template v-if="previewPlan.directional">
                <div
                  class="pointer-events-none absolute inset-0 z-20"
                  :class="previewPlan.directional.lit.className"
                  :style="previewPlan.directional.lit.style"
                ></div>
                <div
                  class="pointer-events-none absolute inset-0 z-20"
                  :class="previewPlan.directional.shadow.className"
                  :style="previewPlan.directional.shadow.style"
                ></div>
              </template>
              <div
                v-if="previewPlan.grade"
                class="pointer-events-none absolute inset-0 z-20"
                :class="previewPlan.grade.className"
                :style="previewPlan.grade.style"
              ></div>
              <div
                v-if="previewPlan.vignette"
                class="pointer-events-none absolute inset-0 z-20"
                :class="previewPlan.vignette.className"
                :style="previewPlan.vignette.style"
              ></div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
  import { computed, nextTick, reactive, ref, watch } from "vue";
  import { useI18n } from "vue-i18n";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { Button } from "../../base";
  import LightingSliderRow from "./LightingSliderRow.vue";
  import LightingColorRow from "./LightingColorRow.vue";
  import type { LightingPreset } from "../../../api/services/lighting";
  import { applyLighting, clearLighting, saveLightingPreset } from "../../../api/services/lighting";
  import type { LightingParams } from "../../../api/services/scene";
  import { useGameStore } from "../../../stores/modules/game";
  import { useLightingStore } from "../../../stores/modules/lighting";
  import { useDialogStore } from "../../../stores/modules/ui/dialog";
  import { EMOTION_CONFIG_EMO } from "../../../controllers/emotion/config";
  import {
    angleTowardLight,
    blankLighting,
    cloneLighting,
    planLighting,
  } from "../../../utils/lighting";

  const props = defineProps<{ show: boolean; editing: LightingPreset | null }>();
  const emit = defineEmits<{ close: []; saved: [id: string] }>();

  const { t } = useI18n();
  const gameStore = useGameStore();
  const lightingStore = useLightingStore();
  const dialogStore = useDialogStore();

  const L = "settings.background.lighting.editor";

  const draft = reactive<LightingParams>(blankLighting());
  const name = ref("");
  const description = ref("");
  const mood = ref("");
  const saving = ref(false);

  /** 打开时的画面状态：取消要还回去，保存则交给父组件套用新预设。 */
  let snapshot: { preset: string | null; params: LightingParams } | null = null;
  /** 打开时那盏灯的参数，「放弃改动」退回到这里。 */
  let initial: LightingParams = blankLighting();
  let dirty = false;
  let savedFlag = false;
  let suspend = false;

  function getPath(path: string): unknown {
    return path.split(".").reduce<any>((o, k) => o?.[k], draft);
  }
  function setPath(path: string, value: unknown) {
    const keys = path.split(".");
    const last = keys.pop() as string;
    let host: any = draft;
    for (const k of keys) host = host[k];
    host[last] = value;
  }

  function readBool(path: string): boolean {
    return !!getPath(path);
  }
  function writeBool(path: string, v: boolean) {
    setPath(path, v);
  }
  function readStr(path: string): string {
    return String(getPath(path) ?? "");
  }
  function writeStr(path: string, v: string) {
    setPath(path, v);
  }

  interface NumRow {
    path: string;
    label: string;
    min: number;
    max: number;
    step: number;
    /** 存储用 0–1、显示用 0–100 的层用 scale 换算，免得两处口径不一致 */
    scale?: number;
    decimals?: number;
    unit?: string;
    color?: string;
  }
  function readNum(r: NumRow): number {
    return Number(getPath(r.path) ?? 0) * (r.scale ?? 1);
  }
  function writeNum(r: NumRow, v: number) {
    const raw = v / (r.scale ?? 1);
    setPath(r.path, Math.round(raw * 1000) / 1000);
  }

  // 灯位是方向光与冷暖分离的角度来源，拖动时同步写回 light_angle，
  // 不让两者各存一份——分开存一定会算出「窗在右上、阴影也在右上」。
  const lightAngle = computed(() => angleTowardLight(draft.light_x, draft.light_y));

  const padRect = ref<DOMRect | null>(null);
  const padStyle = computed(() => ({
    background: `radial-gradient(circle at ${draft.light_x}% ${draft.light_y}%, ${
      draft.overlay_color1
    } 0%, rgba(0,0,0,0) 60%), linear-gradient(${Math.round(
      lightAngle.value
    )}deg, ${draft.shadow_cool_color} 0%, rgba(0,0,0,0) 55%, ${draft.light_warm_color} 100%)`,
  }));

  function moveLight(e: PointerEvent) {
    const rect = padRect.value;
    if (!rect) return;
    const x = Math.round(((e.clientX - rect.left) / rect.width) * 100);
    const y = Math.round(((e.clientY - rect.top) / rect.height) * 100);
    draft.light_x = Math.min(100, Math.max(0, x));
    draft.light_y = Math.min(100, Math.max(0, y));
    draft.light_angle = lightAngle.value;
  }
  function startDrag(e: PointerEvent) {
    const el = e.currentTarget as HTMLElement;
    padRect.value = el.getBoundingClientRect();
    // 捕获指针：拖到板子外面也继续跟着手指走，不会一滑就断
    el.setPointerCapture?.(e.pointerId);
    moveLight(e);
  }
  function onDrag(e: PointerEvent) {
    if (padRect.value) moveLight(e);
  }
  function endDrag() {
    padRect.value = null;
  }

  interface SelectRow {
    path: string;
    label: string;
    options: { value: string; label: string }[];
  }
  interface ColorRow {
    path: string;
    label: string;
  }
  interface Group {
    title?: string;
    rows?: NumRow[];
    colors?: ColorRow[];
  }
  interface Section {
    key: string;
    title: string;
    /** 该层的启用位路径；有值时未启用就把整组控件压暗并锁住 */
    enable?: string;
    tip?: string;
    pad?: boolean;
    selects?: SelectRow[];
    groups?: Group[];
  }

  /** 一行配置表撑起整块面板：加一层只改这里，不再复制粘贴一段滑块。 */
  const sections = computed<Section[]>(() => {
    const pct = { scale: 100, unit: "%", decimals: 0 };
    const key = (k: string) => t(`${L}.row.${k}`);
    const ckey = (k: string) => t(`${L}.color.${k}`);
    return [
      {
        key: "position",
        title: t(`${L}.section.position`),
        tip: t(`${L}.positionTip`),
        pad: true,
      },
      {
        key: "overlay",
        title: t(`${L}.section.overlay`),
        enable: "overlay_enabled",
        selects: [
          {
            path: "blend_mode",
            label: t(`${L}.blend`),
            options: [
              "normal",
              "multiply",
              "screen",
              "overlay",
              "soft-light",
              "hard-light",
              "color-dodge",
              "color-burn",
              "difference",
            ].map((v) => ({ value: v, label: v })),
          },
          {
            path: "overlay_target",
            label: t(`${L}.target`),
            options: [
              { value: "both", label: t("settings.sceneEdit.overlayTarget.both") },
              { value: "character", label: t("settings.sceneEdit.overlayTarget.character") },
              { value: "background", label: t("settings.sceneEdit.overlayTarget.background") },
            ],
          },
        ],
        groups: [
          {
            rows: [
              {
                path: "overlay_radius",
                label: key("overlayRadius"),
                min: 10,
                max: 100,
                step: 1,
                unit: "%",
              },
              {
                path: "overlay_opacity",
                label: key("overlayOpacity"),
                min: 0,
                max: 100,
                step: 1,
                ...pct,
              },
            ] as NumRow[],
            colors: [
              { path: "overlay_color1", label: ckey("centerColor") },
              { path: "overlay_color2", label: ckey("edgeColor") },
            ],
          },
        ],
      },
      {
        key: "directional",
        title: t(`${L}.section.directional`),
        enable: "directional_enabled",
        tip: t(`${L}.directionalTip`),
        groups: [
          {
            rows: [
              {
                path: "light_strength",
                label: key("lightStrength"),
                min: 0,
                max: 100,
                step: 1,
                ...pct,
              },
              {
                path: "light_softness",
                label: key("lightSoftness"),
                min: 0,
                max: 100,
                step: 1,
                ...pct,
              },
            ] as NumRow[],
            colors: [
              { path: "light_warm_color", label: ckey("warmColor") },
              { path: "shadow_cool_color", label: ckey("coolColor") },
            ],
          },
        ],
      },
      {
        key: "rim",
        title: t(`${L}.section.rim`),
        enable: "character.rim_enabled",
        tip: t(`${L}.rimTip`),
        groups: [
          {
            rows: [
              {
                path: "character.rim_dx",
                label: key("rimDx"),
                min: -40,
                max: 40,
                step: 1,
                unit: "px",
              },
              {
                path: "character.rim_dy",
                label: key("rimDy"),
                min: -40,
                max: 40,
                step: 1,
                unit: "px",
              },
              {
                path: "character.rim_blur",
                label: key("rimBlur"),
                min: 0,
                max: 40,
                step: 1,
                unit: "px",
              },
            ] as NumRow[],
            colors: [{ path: "character.rim_color", label: ckey("rimColor") }],
          },
        ],
      },
      {
        key: "bloom",
        title: t(`${L}.section.bloom`),
        enable: "bloom_enabled",
        groups: [
          {
            rows: [
              {
                path: "bloom_radius",
                label: key("bloomRadius"),
                min: 2,
                max: 60,
                step: 1,
                unit: "px",
              },
              {
                path: "bloom_intensity",
                label: key("bloomIntensity"),
                min: 0,
                max: 100,
                step: 1,
                ...pct,
              },
            ] as NumRow[],
          },
        ],
      },
      {
        key: "vignette",
        title: t(`${L}.section.vignette`),
        enable: "vignette_enabled",
        groups: [
          {
            rows: [
              {
                path: "vignette_strength",
                label: key("vignetteStrength"),
                min: 0,
                max: 100,
                step: 1,
                ...pct,
              },
              {
                path: "vignette_size",
                label: key("vignetteSize"),
                min: 0,
                max: 95,
                step: 1,
                unit: "%",
              },
            ] as NumRow[],
          },
        ],
      },
      {
        key: "grade",
        title: t(`${L}.section.grade`),
        enable: "grade_enabled",
        groups: [
          {
            rows: [
              {
                path: "grade_strength",
                label: key("gradeStrength"),
                min: 0,
                max: 100,
                step: 1,
                ...pct,
              },
            ] as NumRow[],
            colors: [
              { path: "grade_warm_color", label: ckey("gradeWarm") },
              { path: "grade_cool_color", label: ckey("gradeCool") },
            ],
          },
        ],
      },
      {
        key: "breathing",
        title: t(`${L}.section.breathing`),
        enable: "breathing_enabled",
        groups: [
          {
            rows: [
              {
                path: "breathing_period",
                label: key("breathingPeriod"),
                min: 1,
                max: 20,
                step: 0.5,
                unit: "s",
                decimals: 1,
              },
              {
                path: "breathing_amount",
                label: key("breathingAmount"),
                min: 0,
                max: 100,
                step: 1,
                ...pct,
              },
            ] as NumRow[],
          },
        ],
      },
      {
        key: "filters",
        title: t(`${L}.section.filters`),
        tip: t(`${L}.filtersTip`),
        groups: [
          {
            title: t("settings.sceneEdit.filter.character"),
            rows: filterRows("character", key),
            colors: [{ path: "character.glow_color", label: ckey("glowColor") }],
          },
          {
            title: t("settings.sceneEdit.filter.background"),
            rows: filterRows("background", key),
            colors: [{ path: "background.glow_color", label: ckey("glowColor") }],
          },
        ],
      },
    ];
  });

  function filterRows(host: "character" | "background", key: (k: string) => string): NumRow[] {
    return [
      {
        path: `${host}.brightness`,
        label: key("brightness"),
        min: 0.3,
        max: 2.2,
        step: 0.01,
        decimals: 2,
        color: "#fbbf24",
      },
      {
        path: `${host}.contrast`,
        label: key("contrast"),
        min: 0.5,
        max: 2,
        step: 0.01,
        decimals: 2,
        color: "#f97316",
      },
      {
        path: `${host}.saturation`,
        label: key("saturation"),
        min: 0,
        max: 2.5,
        step: 0.01,
        decimals: 2,
        color: "#34d399",
      },
      {
        path: `${host}.sepia`,
        label: key("sepia"),
        min: 0,
        max: 100,
        step: 1,
        scale: 100,
        unit: "%",
        color: "#a78bfa",
      },
      {
        path: `${host}.glow_radius`,
        label: key("glowRadius"),
        min: 0,
        max: 50,
        step: 1,
        unit: "px",
        color: "#60a5fa",
      },
    ];
  }

  // ---- 实时预览：走运行时覆盖通道，主窗口和投屏窗口都会跟着变 ----

  const previewPlan = computed(() => planLighting(draft));

  let timer: ReturnType<typeof setTimeout> | undefined;
  watch(
    () => draft,
    () => {
      if (suspend) return;
      dirty = true;
      clearTimeout(timer);
      timer = setTimeout(pushPreview, 120);
    },
    { deep: true }
  );

  async function pushPreview() {
    try {
      await applyLighting({ params: buildParams() });
    } catch (e) {
      console.warn("[Lighting] 预览推送失败:", e);
    }
  }

  function buildParams(): LightingParams {
    return {
      ...cloneLighting(draft),
      light_angle: Math.round(angleTowardLight(draft.light_x, draft.light_y) * 10) / 10,
    };
  }

  // ---- 预览素材：拿当前场景背景 + 立绘，没有就只按纯色底看 ----

  const bgSrc = computed(() => {
    const bg = gameStore.currentScene?.background;
    return bg ? convertFileSrc(bg) : "";
  });
  const avatarSrc = ref("");

  async function resolveAvatar() {
    avatarSrc.value = "";
    const roleId = gameStore.currentInteractRoleId;
    const role = roleId ? gameStore.gameRoles[roleId] : null;
    if (!role) return;
    const clothesName =
      role.clothesName === "默认" || !role.clothesName ? "default" : role.clothesName;
    try {
      const path: string = await invoke("get_avatar_file", {
        characterFolder: role.character_folder,
        emotion: EMOTION_CONFIG_EMO[role.emotion] || "正常",
        clothesName,
      });
      avatarSrc.value = convertFileSrc(path);
    } catch {
      avatarSrc.value = "";
    }
  }

  // ---- 打开 / 关闭 ----

  watch(
    () => props.show,
    (v) => {
      if (!v) return;
      suspend = true;
      savedFlag = false;
      dirty = false;
      const from = props.editing?.params ?? lightingStore.baseParams ?? blankLighting();
      // 两份独立克隆：共用一个对象会让改 draft 连带改掉「放弃改动」的基准。
      initial = cloneLighting(from);
      Object.assign(draft, cloneLighting(from));
      name.value = props.editing?.name ?? "";
      description.value = props.editing?.description ?? "";
      mood.value = (props.editing?.mood ?? []).join(" ");
      const ov = lightingStore.override;
      snapshot = ov ? { preset: ov.preset, params: cloneLighting(ov.params) } : null;
      resolveAvatar();
      // 等这一轮 deep watch 走完再解锁，否则初始化会被当成用户改动而推出预览
      void nextTick(() => {
        suspend = false;
      });
    }
  );

  function resetAll() {
    Object.assign(draft, blankLighting());
  }

  /** 退回打开编辑器时那盏灯：编辑已有预设时不能退到「当前全局预设」，那是另一盏。 */
  function revert() {
    Object.assign(draft, cloneLighting(initial));
  }

  async function close() {
    clearTimeout(timer);
    if (savedFlag) {
      emit("close");
      return;
    }
    if (dirty) {
      try {
        if (snapshot) await applyLighting({ preset: snapshot.preset, params: snapshot.params });
        else await clearLighting();
      } catch (e) {
        console.error("[Lighting] 还原预览前状态失败:", e);
      }
    }
    emit("close");
  }

  async function save() {
    const trimmed = name.value.trim();
    if (!trimmed) {
      dialogStore.alert(t(`${L}.nameRequired`));
      return;
    }
    saving.value = true;
    try {
      const saved = await saveLightingPreset({
        name: trimmed,
        description: description.value.trim(),
        mood: mood.value
          .split(/[,，\s]+/)
          .map((s) => s.trim())
          .filter(Boolean)
          .slice(0, 8),
        params: buildParams(),
        replaceId: props.editing?.id ?? null,
      });
      await lightingStore.refreshPresets();
      savedFlag = true;
      emit("saved", saved.id);
    } catch (e) {
      dialogStore.alert(t(`${L}.saveFailed`, { msg: String(e) }));
    } finally {
      saving.value = false;
    }
  }
</script>

<style scoped>
  .modal-enter-active,
  .modal-leave-active {
    transition: opacity 0.25s ease;
  }

  .modal-enter-from,
  .modal-leave-to {
    opacity: 0;
  }
</style>

<template>
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
      <span class="text-xs font-bold tracking-wider text-white/80">{{ sec.title }}</span>
    </label>
    <div v-if="sec.tip" class="mt-1 text-[11px] leading-snug text-white/35">{{ sec.tip }}</div>

    <div
      class="mt-2 space-y-2"
      :class="{ 'pointer-events-none opacity-35': sec.enable && !readBool(sec.enable) }"
    >
      <!-- 灯位拖拽板 -->
      <div
        v-if="sec.pad"
        class="relative h-28 w-full cursor-crosshair touch-none overflow-hidden rounded-lg border border-white/15 select-none"
        :style="padStyle"
        @pointerdown="startDrag"
        @pointermove="onDrag"
        @pointerup="endDrag"
        @pointercancel="endDrag"
      >
        <div
          class="pointer-events-none absolute h-5 w-5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white bg-amber-300 shadow-[0_0_14px_4px_rgba(252,211,77,0.8)]"
          :style="{ left: `${params.light_x}%`, top: `${params.light_y}%` }"
        ></div>
      </div>
      <div v-if="sec.pad" class="flex items-center gap-3 text-[11px] text-white/45">
        <span>{{ $t("settings.background.lighting.editor.angle") }}</span>
        <span class="font-bold text-amber-300/90 tabular-nums">{{ Math.round(lightAngle) }}°</span>
        <span class="text-white/25">·</span>
        <span class="tabular-nums">{{ params.light_x }}% / {{ params.light_y }}%</span>
      </div>

      <div v-for="sel in sec.selects" :key="sel.path" class="flex items-center gap-2">
        <span class="w-[5.5rem] shrink-0 text-[11px] text-white/50">{{ sel.label }}</span>
        <select
          class="min-w-0 flex-1 rounded-lg border border-white/10 bg-black/40 px-2 py-1 text-[11px] text-white focus:border-amber-400/60 focus:outline-none"
          :value="readStr(sel.path)"
          @change="writeStr(sel.path, ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="o in sel.options" :key="o.value" :value="o.value">{{ o.label }}</option>
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
</template>

<script setup lang="ts">
/**
 * 光影各层的调参控件。全局预设编辑器（LightingEditorModal）和场景编辑器
 * （SceneEditModal）共用这一份：两边以前各有各的滑块，场景那边只认最早 11 个
 * 字段，进阶层既看不到也调不动，保存时还会把进阶字段整个抹掉。
 *
 * `params` 传进来的是父组件自己的 reactive 草稿，这里直接改它的属性（不重新
 * 赋值对象），所以父组件的 deep watch 照常触发，不需要 emit。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import LightingSliderRow from "./LightingSliderRow.vue";
import LightingColorRow from "./LightingColorRow.vue";
import type { LightingParams } from "@/api/services/scene";
import { angleTowardLight } from "@/utils/lighting";

const props = defineProps<{ params: LightingParams }>();

const { t } = useI18n();
const L = "settings.background.lighting.editor";

function getPath(path: string): unknown {
  return path.split(".").reduce<any>((o, k) => o?.[k], props.params);
}
function setPath(path: string, value: unknown) {
  const keys = path.split(".");
  const last = keys.pop() as string;
  let host: any = props.params;
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
const lightAngle = computed(() => angleTowardLight(props.params.light_x, props.params.light_y));

const padRect = ref<DOMRect | null>(null);
const padStyle = computed(() => ({
  background: `radial-gradient(circle at ${props.params.light_x}% ${props.params.light_y}%, ${
    props.params.overlay_color1
  } 0%, rgba(0,0,0,0) 60%), linear-gradient(${Math.round(
    lightAngle.value,
  )}deg, ${props.params.shadow_cool_color} 0%, rgba(0,0,0,0) 55%, ${
    props.params.light_warm_color
  } 100%)`,
}));

function moveLight(e: PointerEvent) {
  const rect = padRect.value;
  if (!rect) return;
  const x = Math.round(((e.clientX - rect.left) / rect.width) * 100);
  const y = Math.round(((e.clientY - rect.top) / rect.height) * 100);
  props.params.light_x = Math.min(100, Math.max(0, x));
  props.params.light_y = Math.min(100, Math.max(0, y));
  props.params.light_angle = lightAngle.value;
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
</script>

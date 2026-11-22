<script setup lang="ts">
/**
 * 选中光源的参数面板。
 *
 * 字段集随灯型变化 —— 这正是沙箱里由 renderParams() 现生成的那段逻辑：
 * 点光/锥光有位置与半径，平行光/锥光有方位与仰角，锥光多锥角与半影。
 */
import { computed, ref } from "vue";
import type { LightSource } from "@/api/services/scene";
import ParamSlider from "./ParamSlider.vue";
import ParamColor from "./ParamColor.vue";

const props = defineProps<{
  light: LightSource | null;
  /** 当前预设里同位置的那盏灯，用于双击复位 */
  baseline?: LightSource | null;
}>();

const emit = defineEmits<{ (e: "patch", patch: Partial<LightSource>): void }>();

const showMore = ref(false);

const TYPE_NAME: Record<LightSource["type"], string> = {
  point: "点光",
  ambient: "环境光",
  directional: "平行光",
  spot: "锥光",
};

const typeName = computed(() => (props.light ? TYPE_NAME[props.light.type] : "光源"));
const hasPos = computed(() => props.light?.type === "point" || props.light?.type === "spot");
const hasDir = computed(() => props.light?.type === "directional" || props.light?.type === "spot");
const isSpot = computed(() => props.light?.type === "spot");

function base(key: keyof LightSource): number | undefined {
  const v = props.baseline?.[key];
  return typeof v === "number" ? v : undefined;
}

function setNum(key: keyof LightSource, v: number) {
  emit("patch", { [key]: v } as Partial<LightSource>);
}

function onPosInput(axis: "x" | "y", e: Event) {
  const raw = (e.target as HTMLInputElement).value;
  const n = Number(raw);
  if (!Number.isFinite(n)) return;
  setNum(axis, +Math.min(1, Math.max(0, n)).toFixed(3));
}
</script>

<template>
  <section class="light-params">
    <header class="lp-head">
      <span class="lp-icon" :style="{ color: light?.color || 'var(--wb-accent)' }">
        <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor">
          <circle cx="8" cy="8" r="3.3" fill="currentColor" />
          <circle cx="8" cy="8" r="5.6" stroke-width="1" opacity="0.4" />
        </svg>
      </span>
      <span class="lp-title">{{ typeName }}</span>
      <button
        type="button"
        class="wb-icon-btn lp-more"
        :class="{ 'is-open': showMore }"
        :title="showMore ? '收起高级参数' : '更多光源参数'"
        @click="showMore = !showMore"
      >
        <svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor">
          <circle cx="3.6" cy="8" r="1.05" />
          <circle cx="8" cy="8" r="1.05" />
          <circle cx="12.4" cy="8" r="1.05" />
        </svg>
      </button>
    </header>

    <p v-if="!light" class="wb-empty">先在左栏选一盏灯</p>

    <template v-else>
      <ParamColor
        label="颜色"
        :model-value="light.color"
        :percent="light.intensity * 100"
        @update:model-value="emit('patch', { color: $event })"
      />
      <ParamSlider
        label="强度"
        :model-value="light.intensity"
        :min="0"
        :max="3"
        :step="0.01"
        :default-value="base('intensity')"
        @update:model-value="setNum('intensity', $event)"
      />

      <!-- 位置：一行两个数字框，稿上的形态 -->
      <div v-if="hasPos" class="wb-param">
        <div class="wb-param-top">
          <span class="wb-param-label">位置</span>
          <span class="wb-param-value" style="gap: 4px">
            <span class="lp-axis">X</span>
            <input
              class="wb-num"
              style="width: 44px; text-align: right"
              :value="light.x.toFixed(2)"
              inputmode="decimal"
              @input="onPosInput('x', $event)"
            />
            <span class="lp-axis">Y</span>
            <input
              class="wb-num"
              style="width: 44px; text-align: right"
              :value="light.y.toFixed(2)"
              inputmode="decimal"
              @input="onPosInput('y', $event)"
            />
          </span>
        </div>
      </div>

      <ParamSlider
        v-if="hasPos"
        label="照射半径"
        :model-value="light.radius"
        :min="0.1"
        :max="2.2"
        :step="0.01"
        :default-value="base('radius')"
        @update:model-value="setNum('radius', $event)"
      />

      <template v-if="hasDir">
        <ParamSlider
          label="方位角"
          :model-value="light.azimuth"
          :min="0"
          :max="360"
          :step="1"
          :precision="0"
          :default-value="base('azimuth')"
          @update:model-value="setNum('azimuth', $event)"
        />
        <ParamSlider
          label="仰角"
          :model-value="light.elevation"
          :min="-10"
          :max="90"
          :step="1"
          :precision="0"
          :default-value="base('elevation')"
          @update:model-value="setNum('elevation', $event)"
        />
      </template>

      <template v-if="isSpot">
        <ParamSlider
          label="锥角"
          :model-value="light.cone"
          :min="8"
          :max="150"
          :step="1"
          :precision="0"
          :default-value="base('cone')"
          @update:model-value="setNum('cone', $event)"
        />
      </template>

      <!-- 高级：软边 / 高度 / 半影 -->
      <template v-if="showMore">
        <ParamSlider
          v-if="hasPos"
          label="边缘柔和"
          :model-value="light.softness"
          :min="0.05"
          :max="1"
          :step="0.01"
          :default-value="base('softness')"
          @update:model-value="setNum('softness', $event)"
        />
        <ParamSlider
          v-if="hasPos"
          label="光源高度"
          :model-value="light.z"
          :min="0.02"
          :max="1.5"
          :step="0.01"
          :default-value="base('z')"
          @update:model-value="setNum('z', $event)"
        />
        <ParamSlider
          v-if="isSpot"
          label="锥边半影"
          :model-value="light.penumbra"
          :min="0.02"
          :max="1"
          :step="0.01"
          :default-value="base('penumbra')"
          @update:model-value="setNum('penumbra', $event)"
        />
      </template>
    </template>
  </section>
</template>

<style scoped>
.light-params {
  border-bottom: 1px solid var(--wb-border);
}

.lp-head {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 10px;
}

.lp-icon {
  display: flex;
  flex-shrink: 0;
}

.lp-title {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--wb-text);
}

.lp-more {
  margin-left: auto;
}

.lp-more.is-open {
  background: var(--wb-accent-weak);
  color: var(--wb-accent);
}

.lp-axis {
  font-size: 10px;
  color: var(--wb-faint);
}

.light-params :deep(.wb-param:first-of-type) {
  padding-top: 2px;
}
</style>

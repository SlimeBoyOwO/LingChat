<script setup lang="ts">
/** 画布下方控制条：显示光源 / 工具条 / 缩放 / 网格。 */
import CanvasToolbar from "./CanvasToolbar.vue";
import type { CanvasTool } from "./useWorkbenchUi";

const props = withDefaults(
  defineProps<{
    showGizmo: boolean;
    showGrid: boolean;
    zoom: number;
    tool: CanvasTool;
    fullscreen?: boolean;
  }>(),
  { zoom: 1, tool: "select" },
);

const emit = defineEmits<{
  (e: "update:showGizmo", v: boolean): void;
  (e: "update:showGrid", v: boolean): void;
  (e: "update:zoom", v: number): void;
  (e: "update:tool", v: CanvasTool): void;
  (e: "toggle-fullscreen"): void;
}>();

const ZOOM_MIN = 0.5;
const ZOOM_MAX = 2;

function stepZoom(delta: number) {
  const next = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, +(props.zoom + delta).toFixed(2)));
  emit("update:zoom", next);
}
</script>

<template>
  <div class="stage-foot">
    <button
      type="button"
      class="stage-foot-btn"
      :class="{ 'is-off': !showGizmo }"
      @click="emit('update:showGizmo', !showGizmo)"
    >
      <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor">
        <path
          d="M1.8 8S4.2 4.2 8 4.2 14.2 8 14.2 8 11.8 11.8 8 11.8 1.8 8 1.8 8Z"
          stroke-width="1.2"
        />
        <circle cx="8" cy="8" r="1.9" stroke-width="1.2" />
      </svg>
      显示光源
    </button>

    <span class="stage-foot-spacer" />

    <!-- 工具条从画布内挪到这里：稿上压在裙摆，会挡立绘 -->
    <div class="stage-foot-tools">
      <CanvasToolbar
        :model-value="tool"
        :fullscreen="fullscreen"
        @update:model-value="emit('update:tool', $event)"
        @toggle-fullscreen="emit('toggle-fullscreen')"
      />
    </div>

    <span class="stage-foot-spacer" />

    <div class="stage-zoom">
      <button type="button" class="stage-zoom-btn" title="缩小" @click="stepZoom(-0.05)">−</button>
      <span class="stage-zoom-val">{{ Math.round(zoom * 100) }}%</span>
      <button type="button" class="stage-zoom-btn" title="放大" @click="stepZoom(0.05)">＋</button>
    </div>

    <button
      type="button"
      class="stage-foot-btn stage-foot-icon"
      :class="{ 'is-on': showGrid }"
      title="网格"
      @click="emit('update:showGrid', !showGrid)"
    >
      <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor">
        <path d="M6 2v12M10 2v12M2 6h12M2 10h12" stroke-width="1.1" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.stage-foot {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 32px;
  padding: 0 10px;
  background: var(--wb-surface);
  border-top: 1px solid var(--wb-border);
  flex-shrink: 0;
}

.stage-foot-tools {
  display: flex;
  align-items: center;
}

.stage-foot-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  height: 20px;
  padding: 0 6px;
  border-radius: 5px;
  font-size: 10.5px;
  color: var(--wb-text-2);
  transition:
    background 0.12s ease,
    color 0.12s ease;
}

.stage-foot-btn:hover {
  background: var(--wb-hover);
  color: var(--wb-text);
}

.stage-foot-btn.is-off {
  color: var(--wb-faint);
}

.stage-foot-btn.is-on {
  color: var(--wb-accent);
  background: var(--wb-accent-weak);
}

.stage-foot-icon {
  padding: 0 5px;
}

.stage-foot-spacer {
  flex: 1;
}

.stage-zoom {
  display: flex;
  align-items: center;
  gap: 1px;
}

.stage-zoom-btn {
  width: 18px;
  height: 18px;
  border-radius: 5px;
  color: var(--wb-text-2);
  font-size: 11px;
  line-height: 1;
}

.stage-zoom-btn:hover {
  background: var(--wb-hover);
  color: var(--wb-text);
}

.stage-zoom-val {
  min-width: 34px;
  text-align: center;
  font-size: 10.5px;
  color: var(--wb-text-2);
  font-variant-numeric: tabular-nums;
}
</style>

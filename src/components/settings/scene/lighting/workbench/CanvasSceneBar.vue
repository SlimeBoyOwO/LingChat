<script setup lang="ts">
/**
 * 画布上方的场景条：场景名（可换本地底图）+ 景别下拉 + 前后对比 + 适应画布。
 *
 * 景别做下拉而不是一排按钮，是因为稿上就是「中景 ▾」；展开态照微交互稿：
 * 白底卡片、当前项紫字 + 浅紫底 + 勾。
 */
import { onUnmounted, ref } from "vue";
import type { LightingShot } from "@/api/services/scene";
import { SHOT_CAMERA_MAP } from "../types";

const props = defineProps<{
  sceneName?: string;
  shot: LightingShot;
}>();

const emit = defineEmits<{
  (e: "set-shot", shot: LightingShot): void;
  (e: "pick-bg"): void;
  (e: "compare-hold", active: boolean): void;
  (e: "fit"): void;
}>();

const shotOpen = ref(false);
const pressing = ref(false);

function toggleShotMenu() {
  shotOpen.value = !shotOpen.value;
  if (shotOpen.value) window.addEventListener("mousedown", closeShotMenu, { once: true });
}

function closeShotMenu(e: MouseEvent) {
  const el = e.target as HTMLElement | null;
  if (el?.closest(".stage-shot-wrap")) return;
  shotOpen.value = false;
}

function pickShot(key: LightingShot) {
  shotOpen.value = false;
  emit("set-shot", key);
}

function setPressing(v: boolean) {
  if (pressing.value === v) return;
  pressing.value = v;
  emit("compare-hold", v);
}

const shotLabel = () => SHOT_CAMERA_MAP[props.shot]?.name ?? "中景";

onUnmounted(() => {
  if (shotOpen.value) window.removeEventListener("mousedown", closeShotMenu);
});
</script>

<template>
  <div class="stage-bar">
    <div class="stage-bar-left">
      <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor">
        <rect x="2" y="3.2" width="12" height="9.6" rx="1.6" stroke-width="1.2" />
        <path d="M2 9.6 5.6 6.4l2.8 2.4 2.2-1.8L14 9.2" stroke-width="1.2" />
      </svg>
      <button type="button" class="stage-scene" title="更换本地场景底图" @click="emit('pick-bg')">
        {{ sceneName || "场景底图" }}
        <svg viewBox="0 0 16 16" width="9" height="9" fill="none" stroke="currentColor">
          <path d="M4 6.5 8 10.5 12 6.5" stroke-width="1.6" stroke-linecap="round" />
        </svg>
      </button>

      <span class="stage-divider" />

      <div class="stage-shot-wrap">
        <button type="button" class="stage-scene" @click.stop="toggleShotMenu">
          {{ shotLabel() }}
          <svg viewBox="0 0 16 16" width="9" height="9" fill="none" stroke="currentColor">
            <path d="M4 6.5 8 10.5 12 6.5" stroke-width="1.6" stroke-linecap="round" />
          </svg>
        </button>
        <div v-if="shotOpen" class="shot-pop">
          <button
            v-for="(v, key) in SHOT_CAMERA_MAP"
            :key="key"
            type="button"
            class="shot-item"
            :class="{ 'is-active': shot === key }"
            @click="pickShot(key)"
          >
            <span class="shot-check">{{ shot === key ? "✓" : "" }}</span>
            {{ v.name }}
          </button>
        </div>
      </div>
    </div>

    <div class="stage-bar-right">
      <button
        type="button"
        class="wb-btn"
        :class="{ 'is-pressing': pressing }"
        @mousedown="setPressing(true)"
        @mouseup="setPressing(false)"
        @mouseleave="setPressing(false)"
      >
        前后对比
      </button>
      <button type="button" class="wb-btn" @click="emit('fit')">适应画布</button>
    </div>
  </div>
</template>

<style scoped>
.stage-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 34px;
  padding: 0 10px;
  background: var(--wb-surface);
  border-bottom: 1px solid var(--wb-border);
  flex-shrink: 0;
  color: var(--wb-muted);
}

.stage-bar-left,
.stage-bar-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.stage-bar-right {
  margin-left: auto;
}

.stage-scene {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 11.5px;
  color: var(--wb-text-2);
}

.stage-scene:hover {
  color: var(--wb-text);
}

.stage-divider {
  width: 1px;
  height: 12px;
  background: var(--wb-border);
}

.stage-shot-wrap {
  position: relative;
}

.shot-pop {
  position: absolute;
  left: 0;
  top: 22px;
  z-index: 30;
  min-width: 96px;
  padding: 4px;
  border-radius: var(--wb-r-md);
  background: var(--wb-surface);
  border: 1px solid var(--wb-border);
  box-shadow: 0 10px 28px rgb(23 23 28 / 16%);
}

.shot-item {
  display: flex;
  align-items: center;
  gap: 4px;
  width: 100%;
  height: 24px;
  padding: 0 6px;
  border-radius: 5px;
  font-size: 11px;
  color: var(--wb-text-2);
  text-align: left;
}

.shot-item:hover {
  background: var(--wb-hover);
  color: var(--wb-text);
}

.shot-item.is-active {
  color: var(--wb-accent);
  font-weight: 600;
  background: var(--wb-accent-weak);
}

.shot-check {
  width: 10px;
  font-size: 9px;
  color: var(--wb-accent);
}

.stage-bar-right :deep(.wb-btn) {
  height: 22px;
  font-size: 11px;
}

.stage-bar-right .wb-btn.is-pressing {
  border-color: var(--wb-accent);
  color: var(--wb-accent);
  background: var(--wb-accent-weak);
}
</style>

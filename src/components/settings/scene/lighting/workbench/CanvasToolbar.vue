<script setup lang="ts">
/**
 * 画布内的浮动工具条：选择 / 手形 / 光源 / 框选，右侧分隔线后是全屏预览。
 * 工具切的是视图态，全屏是动作，所以走单独的事件。
 */
export type CanvasTool = "select" | "hand" | "light" | "marquee";

defineProps<{ modelValue: CanvasTool; fullscreen?: boolean }>();
const emit = defineEmits<{
  (e: "update:modelValue", v: CanvasTool): void;
  (e: "toggle-fullscreen"): void;
}>();

const TOOLS: { key: CanvasTool; label: string; path: string }[] = [
  { key: "select", label: "选择", path: "M4 2.5 12 8l-3.4 1.1L7 13 4 2.5Z" },
  {
    key: "hand",
    label: "平移画面",
    path: "M6 7V4.4a1.1 1.1 0 0 1 2.2 0V7m0-.6a1.1 1.1 0 0 1 2.2 0V7m0-.4a1.1 1.1 0 0 1 2.2 0v3.2A4 4 0 0 1 8.6 14h-.4A4.2 4.2 0 0 1 4 9.8V7.4a1.1 1.1 0 0 1 2.2 0",
  },
  {
    key: "light",
    label: "放置光源",
    path: "M8 2v2.2M8 11.8V14M2 8h2.2M11.8 8H14M4 4l1.5 1.5M10.5 10.5 12 12M12 4l-1.5 1.5M5.5 10.5 4 12",
  },
  {
    key: "marquee",
    label: "框选",
    path: "M3 6V4.2A1.2 1.2 0 0 1 4.2 3H6M10 3h1.8A1.2 1.2 0 0 1 13 4.2V6M13 10v1.8A1.2 1.2 0 0 1 11.8 13H10M6 13H4.2A1.2 1.2 0 0 1 3 11.8V10",
  },
];
</script>

<template>
  <div class="canvas-toolbar">
    <button
      v-for="t in TOOLS"
      :key="t.key"
      type="button"
      class="canvas-tool"
      :class="{ 'is-active': modelValue === t.key }"
      :title="t.label"
      @click="emit('update:modelValue', t.key)"
    >
      <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor">
        <path :d="t.path" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </button>

    <span class="canvas-toolbar-sep" />

    <!-- 全屏预览：放在框选右侧（用户指定位置） -->
    <button
      type="button"
      class="canvas-tool"
      :class="{ 'is-active': fullscreen }"
      :title="fullscreen ? '退出全屏 (Esc)' : '全屏预览立绘与场景'"
      @click="emit('toggle-fullscreen')"
    >
      <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor">
        <path
          v-if="!fullscreen"
          d="M6 2.5H2.5V6M10 2.5h3.5V6M6 13.5H2.5V10M10 13.5h3.5V10"
          stroke-width="1.35"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
        <path
          v-else
          d="M2.5 6H6V2.5M13.5 6H10V2.5M2.5 10H6v3.5M13.5 10H10v3.5"
          stroke-width="1.35"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.canvas-toolbar {
  display: flex;
  align-items: center;
  gap: 1px;
  padding: 2px;
  border-radius: 999px;
  background: var(--wb-bg);
  border: 1px solid var(--wb-border);
}

.canvas-toolbar-sep {
  width: 1px;
  height: 14px;
  margin: 0 2px;
  background: var(--wb-border);
  flex-shrink: 0;
}

.canvas-tool {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: 999px;
  color: var(--wb-text-2);
  transition:
    background 0.12s ease,
    color 0.12s ease;
}

.canvas-tool:hover {
  background: var(--wb-hover);
  color: var(--wb-text);
}

.canvas-tool.is-active {
  background: var(--wb-accent);
  color: #fff;
}
</style>

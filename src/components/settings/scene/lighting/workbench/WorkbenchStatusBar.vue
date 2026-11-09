<script setup lang="ts">
/** 底部状态栏：版本 · 操作提示 · 实时预览信息。 */
withDefaults(defineProps<{ frameRate?: number; resolution?: string }>(), {
  frameRate: 60,
  resolution: "1920 × 1080",
});
</script>

<template>
  <footer class="status">
    <span class="status-left">LingChat 2.5 光影工作台 · v4.0</span>
    <span class="status-mid">拖动画面中的光源调整位置 · 双击角色快速聚焦 · Ctrl+滚轮缩放</span>
    <span class="status-right">
      <span class="status-dot" />
      实时预览 {{ resolution }} · {{ frameRate }} FPS
    </span>
  </footer>
</template>

<style scoped>
.status {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 22px;
  padding: 0 12px;
  background: var(--wb-bg);
  border-top: 1px solid var(--wb-border);
  font-size: 10px;
  color: var(--wb-muted);
  flex-shrink: 0;
}

.status-left {
  flex-shrink: 0;
}

.status-mid {
  flex: 1;
  text-align: center;
  color: var(--wb-faint);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-right {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
}

.status-dot {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: #3fbf7f;
}
</style>

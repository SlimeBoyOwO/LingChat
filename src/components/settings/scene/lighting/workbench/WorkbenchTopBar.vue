<script setup lang="ts">
/** 顶栏：应用标识 + 场景名 + 保存态 + 居中 tab + 撤销/导入/导出。 */
defineProps<{
  sceneName?: string;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
}>();

const emit = defineEmits<{
  (e: "undo"): void;
  (e: "redo"): void;
  (e: "import"): void;
  (e: "export"): void;
  (e: "close"): void;
  (e: "switch-tab", tab: "lighting" | "assets"): void;
}>();
</script>

<template>
  <header class="top">
    <div class="top-left">
      <span class="top-logo">LC</span>
      <span class="top-brand">
        LingChat
        <svg viewBox="0 0 16 16" width="9" height="9" fill="none" stroke="currentColor">
          <path d="M4 6.5 8 10.5 12 6.5" stroke-width="1.6" stroke-linecap="round" />
        </svg>
      </span>

      <span class="top-sep" />

      <span class="top-scene wb-ellipsis">{{ sceneName || "未命名场景" }}</span>

      <span class="top-save" :class="{ 'is-dirty': dirty }">
        <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor">
          <path
            d="M4.6 11.6a3.3 3.3 0 0 1-.3-6.6 4.2 4.2 0 0 1 8 1.1 2.9 2.9 0 0 1-.6 5.5"
            stroke-width="1.2"
            stroke-linecap="round"
          />
        </svg>
        {{ dirty ? "未保存" : "已保存" }}
      </span>
    </div>

    <nav class="top-tabs">
      <button type="button" class="top-tab is-active" @click="emit('switch-tab', 'lighting')">
        光影编辑
      </button>
      <button
        type="button"
        class="top-tab"
        disabled
        title="素材管理尚未接入"
        @click="emit('switch-tab', 'assets')"
      >
        素材管理
      </button>
    </nav>

    <div class="top-right">
      <div class="top-undo">
        <button
          type="button"
          class="wb-icon-btn top-ic"
          title="撤销 (Ctrl+Z)"
          :disabled="!canUndo"
          @click="emit('undo')"
        >
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor">
            <path d="M6.2 4.4 3 7.6l3.2 3.2" stroke-width="1.4" stroke-linecap="round" />
            <path d="M3.2 7.6h6.2a3.4 3.4 0 0 1 0 6.8" stroke-width="1.4" stroke-linecap="round" />
          </svg>
        </button>
        <button
          type="button"
          class="wb-icon-btn top-ic"
          title="重做 (Ctrl+Shift+Z)"
          :disabled="!canRedo"
          @click="emit('redo')"
        >
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor">
            <path d="M9.8 4.4 13 7.6l-3.2 3.2" stroke-width="1.4" stroke-linecap="round" />
            <path d="M12.8 7.6H6.6a3.4 3.4 0 0 0 0 6.8" stroke-width="1.4" stroke-linecap="round" />
          </svg>
        </button>
      </div>

      <button type="button" class="wb-btn" @click="emit('import')">
        <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor">
          <path d="M8 2.8v6.4M5.4 6.8 8 9.4l2.6-2.6" stroke-width="1.3" stroke-linecap="round" />
          <path
            d="M3 11.4v.8A1.4 1.4 0 0 0 4.4 13.6h7.2A1.4 1.4 0 0 0 13 12.2v-.8"
            stroke-width="1.3"
          />
        </svg>
        导入
      </button>
      <button type="button" class="wb-btn wb-btn-primary" @click="emit('export')">
        <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor">
          <path d="M8 9.4V3M5.4 5.6 8 3l2.6 2.6" stroke-width="1.3" stroke-linecap="round" />
          <path
            d="M3 11.4v.8A1.4 1.4 0 0 0 4.4 13.6h7.2A1.4 1.4 0 0 0 13 12.2v-.8"
            stroke-width="1.3"
          />
        </svg>
        导出
      </button>

      <button type="button" class="wb-icon-btn top-close" title="关闭" @click="emit('close')">
        ✕
      </button>
    </div>
  </header>
</template>

<style scoped>
.top {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 44px;
  padding: 0 12px;
  background: var(--wb-surface);
  border-bottom: 1px solid var(--wb-border);
  flex-shrink: 0;
}

.top-left {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
  flex: 1;
}

.top-logo {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 6px;
  background: linear-gradient(145deg, #2a2438, #14121b);
  color: #fff;
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.02em;
  flex-shrink: 0;
}

.top-brand {
  display: flex;
  align-items: center;
  gap: 2px;
  font-size: 12px;
  font-weight: 600;
  color: var(--wb-text);
  flex-shrink: 0;
}

.top-sep {
  width: 1px;
  height: 12px;
  background: var(--wb-border);
  flex-shrink: 0;
}

.top-scene {
  font-size: 11.5px;
  color: var(--wb-text-2);
  max-width: 140px;
}

.top-save {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 10.5px;
  color: var(--wb-muted);
  flex-shrink: 0;
}

.top-save.is-dirty {
  color: #c98a1a;
}

.top-tabs {
  display: flex;
  gap: 18px;
  flex-shrink: 0;
}

.top-tab {
  position: relative;
  height: 44px;
  font-size: 12px;
  color: var(--wb-muted);
  transition: color 0.12s ease;
}

.top-tab:hover:not(:disabled) {
  color: var(--wb-text);
}

.top-tab:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.top-tab.is-active {
  color: var(--wb-text);
  font-weight: 600;
}

.top-tab.is-active::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 2px;
  border-radius: 2px 2px 0 0;
  background: var(--wb-accent);
}

.top-right {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  justify-content: flex-end;
}

.top-undo {
  display: flex;
  align-items: center;
  gap: 1px;
  padding-right: 6px;
  margin-right: 2px;
  border-right: 1px solid var(--wb-border);
}

.top-ic {
  width: 22px;
  height: 22px;
}

.top-close {
  width: 22px;
  height: 22px;
  font-size: 11px;
  margin-left: 2px;
}

.top-close:hover {
  background: #fdecec;
  color: var(--wb-danger);
}
</style>

<script setup lang="ts">
/** 历史记录页：撤销栈是快照式的，没有逐步动作名，所以只给可撤销步数与整档操作。 */
defineProps<{
  canUndo: boolean;
  canRedo: boolean;
  isDirty: boolean;
  presetName?: string;
}>();

const emit = defineEmits<{
  (e: "undo"): void;
  (e: "redo"): void;
  (e: "revert-preset"): void;
}>();
</script>

<template>
  <div class="history">
    <div class="history-block">
      <p class="history-title">撤销历史</p>
      <div class="history-actions">
        <button type="button" class="wb-btn" :disabled="!canUndo" @click="emit('undo')">
          ↶ 撤销
        </button>
        <button type="button" class="wb-btn" :disabled="!canRedo" @click="emit('redo')">
          ↷ 重做
        </button>
      </div>
      <p class="history-note">
        参数改动按快照入栈（上限 30 步），Ctrl+Z / Ctrl+Shift+Z 同样可用。
        历史里不记录动作名称，所以这里只按步数呈现。
      </p>
    </div>

    <div class="history-block">
      <p class="history-title">当前状态</p>
      <div class="history-row">
        <span>预设</span>
        <span class="history-val">{{ presetName || "自定义" }}</span>
      </div>
      <div class="history-row">
        <span>状态</span>
        <span class="history-val">{{ isDirty ? "已修改" : "未修改" }}</span>
      </div>
      <button
        type="button"
        class="wb-btn wb-btn-block"
        style="margin-top: 8px"
        @click="emit('revert-preset')"
      >
        回到该预设参数
      </button>
    </div>
  </div>
</template>

<style scoped>
.history {
  padding: 8px 10px;
}

.history-block + .history-block {
  margin-top: 14px;
  padding-top: 11px;
  border-top: 1px solid var(--wb-border);
}

.history-title {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--wb-text);
  margin-bottom: 6px;
}

.history-actions {
  display: flex;
  gap: 6px;
}

.history-note {
  margin-top: 7px;
  font-size: 10px;
  line-height: 1.6;
  color: var(--wb-muted);
}

.history-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 22px;
  font-size: 11px;
  color: var(--wb-text-2);
}

.history-val {
  color: var(--wb-text);
  font-weight: 500;
}
</style>

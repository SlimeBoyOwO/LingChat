<script setup lang="ts">
/**
 * 预设列表的折叠控制（列表末尾一行）。
 *
 * 折叠时是一个居中的 `⋯`：点一下**直接把列表展开成全部**（不弹菜单）；
 * 展开后这一行变成「收起 ⌃」，点一下回到只显示 4 个。
 */
defineProps<{
  /** true = 只显示前 4 个 */
  collapsed: boolean;
}>();

const emit = defineEmits<{ (e: "toggle"): void }>();
</script>

<template>
  <div class="preset-fold">
    <button
      type="button"
      class="preset-fold-btn"
      :title="collapsed ? '展开全部预设' : '收起，只显示 4 个'"
      @click="emit('toggle')"
    >
      <template v-if="collapsed">
        <svg viewBox="0 0 16 16" width="15" height="15" fill="currentColor">
          <circle cx="3.4" cy="8" r="1.25" />
          <circle cx="8" cy="8" r="1.25" />
          <circle cx="12.6" cy="8" r="1.25" />
        </svg>
      </template>
      <template v-else>
        <span class="fold-text">收起</span>
        <svg
          class="fold-chevron"
          viewBox="0 0 16 16"
          width="9"
          height="9"
          fill="none"
          stroke="currentColor"
        >
          <path d="M4 6.5 8 10.5 12 6.5" stroke-width="1.6" stroke-linecap="round" />
        </svg>
      </template>
    </button>
  </div>
</template>

<style scoped>
.preset-fold {
  display: flex;
  justify-content: center;
  align-items: center;
  padding: 2px 6px 6px;
  flex-shrink: 0;
}

.preset-fold-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  height: 22px;
  padding: 0 14px;
  border-radius: 999px;
  color: var(--wb-muted);
  transition:
    background 0.12s ease,
    color 0.12s ease;
}

.preset-fold-btn:hover {
  background: var(--wb-hover);
  color: var(--wb-text-2);
}

.fold-text {
  font-size: 10.5px;
}

.fold-chevron {
  transform: rotate(180deg);
}
</style>

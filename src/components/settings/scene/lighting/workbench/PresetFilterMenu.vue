<script setup lang="ts">
/**
 * 分类筛选器：预设区标题行右侧的「筛选 ▾」+ 弹出菜单。
 *
 * 每项带该分类的预设数量，当前分类打勾——和景别下拉同一套视觉。
 * 点「全部预设」回无筛选。
 */
import { onUnmounted, ref } from "vue";

export interface FilterOption {
  key: string;
  label: string;
  count: number;
}

const props = defineProps<{
  options: FilterOption[];
  modelValue: string;
  /** 当前分类的总数（折叠后也要能看到） */
  total: number;
}>();

const emit = defineEmits<{ (e: "update:modelValue", key: string): void }>();

const open = ref(false);

function toggle() {
  open.value = !open.value;
  if (open.value) window.addEventListener("mousedown", closeOnOutside, { once: true });
}

function closeOnOutside(e: MouseEvent) {
  const el = e.target as HTMLElement | null;
  if (el?.closest(".filter-wrap")) return;
  open.value = false;
}

function pick(key: string) {
  emit("update:modelValue", key);
  open.value = false;
}

const currentLabel = () =>
  props.options.find((o) => o.key === props.modelValue)?.label ?? "全部预设";

onUnmounted(() => window.removeEventListener("mousedown", closeOnOutside));
</script>

<template>
  <div class="filter-wrap">
    <button
      type="button"
      class="filter-btn"
      :class="{ 'is-open': open, 'is-filtered': modelValue !== 'all' }"
      title="按分类筛选预设"
      @click.stop="toggle"
    >
      <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor">
        <path d="M2.5 4h11M4.5 8h7M6.8 12h2.4" stroke-width="1.5" stroke-linecap="round" />
      </svg>
      <span class="filter-label">{{ currentLabel() }}</span>
      <svg
        class="filter-chevron"
        :class="{ 'is-up': open }"
        viewBox="0 0 16 16"
        width="9"
        height="9"
        fill="none"
        stroke="currentColor"
      >
        <path d="M4 6.5 8 10.5 12 6.5" stroke-width="1.6" stroke-linecap="round" />
      </svg>
    </button>

    <div v-if="open" class="filter-mask" @click="open = false" />
    <div v-if="open" class="filter-pop">
      <button
        v-for="o in options"
        :key="o.key"
        type="button"
        class="filter-item"
        :class="{ 'is-active': o.key === modelValue }"
        @click="pick(o.key)"
      >
        <span class="filter-check">{{ o.key === modelValue ? "✓" : "" }}</span>
        <span class="filter-name">{{ o.label }}</span>
        <span class="filter-count">{{ o.key === "all" ? total : o.count }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.filter-wrap {
  position: relative;
}

.filter-btn {
  display: flex;
  align-items: center;
  gap: 3px;
  height: 20px;
  padding: 0 6px;
  border-radius: var(--wb-r-sm);
  font-size: 10.5px;
  color: var(--wb-muted);
  transition:
    background 0.12s ease,
    color 0.12s ease;
}

.filter-btn:hover {
  background: var(--wb-hover);
  color: var(--wb-text-2);
}

.filter-btn.is-open {
  background: var(--wb-accent-weak);
  color: var(--wb-accent);
}

.filter-btn.is-filtered {
  color: var(--wb-accent);
}

.filter-chevron {
  transition: transform 0.12s ease;
}

.filter-chevron.is-up {
  transform: rotate(180deg);
}

.filter-mask {
  position: fixed;
  inset: 0;
  z-index: 15;
}

.filter-pop {
  position: absolute;
  right: 0;
  top: 22px;
  z-index: 16;
  min-width: 132px;
  padding: 4px;
  border-radius: var(--wb-r-md);
  background: var(--wb-surface);
  border: 1px solid var(--wb-border);
  box-shadow: 0 10px 28px rgb(23 23 28 / 16%);
}

.filter-item {
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

.filter-item:hover {
  background: var(--wb-hover);
  color: var(--wb-text);
}

.filter-item.is-active {
  color: var(--wb-accent);
  background: var(--wb-accent-weak);
  font-weight: 600;
}

.filter-check {
  width: 10px;
  flex-shrink: 0;
  font-size: 9px;
  color: var(--wb-accent);
}

.filter-name {
  flex: 1;
}

.filter-count {
  font-size: 10px;
  color: var(--wb-faint);
  font-variant-numeric: tabular-nums;
}
</style>

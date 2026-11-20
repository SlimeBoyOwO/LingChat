<script setup lang="ts">
/**
 * 预设库：搜索 + 筛选 + 卡片列表。
 *
 * 三条规则（都来自实际验收）：
 * 1. 默认只显示 4 张，列表末尾一个 `⋯` 展开成全部，再点变「收起」。
 * 2. 分类不做「点一下轮换」，改成标题行右侧的筛选器菜单。
 * 3. 当前预设不在前 4 时自动顶到第 4 格，避免「选中了看不见的预设」。
 */
import { computed, ref } from "vue";
import type { LightingPreset } from "../types";
import PresetFoldMenu from "./PresetFoldMenu.vue";
import PresetFilterMenu, { type FilterOption } from "./PresetFilterMenu.vue";

const props = defineProps<{
  presets: LightingPreset[];
  curPresetId: string;
  favoriteIds: string[];
}>();

const emit = defineEmits<{
  (e: "select", id: string): void;
  (e: "toggle-favorite", id: string): void;
  (e: "save-as"): void;
}>();

type CatKey = "all" | "daily" | "atmosphere" | "drama" | "dark";

/** 折叠时保留的预设张数 */
const VISIBLE_LIMIT = 4;

const CATS: { key: CatKey; label: string }[] = [
  { key: "all", label: "全部预设" },
  { key: "daily", label: "日常" },
  { key: "atmosphere", label: "氛围" },
  { key: "drama", label: "戏剧" },
  { key: "dark", label: "暗调" },
];

const tab = ref<"lib" | "fav">("lib");
const query = ref("");
const cat = ref<CatKey>("all");
const collapsed = ref(true);

/** 「01·标准中性」→「标准中性」：编号留给排序，不占版面 */
function shortName(name: string): string {
  return name.replace(/^\s*\d+\s*[·.、]\s*/, "");
}

const pool = computed(() =>
  tab.value === "fav"
    ? props.presets.filter((p) => props.favoriteIds.includes(p.id))
    : props.presets,
);

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  return pool.value.filter((p) => {
    if (cat.value !== "all" && p.cat !== cat.value) return false;
    if (!q) return true;
    return p.name.toLowerCase().includes(q) || (p.desc || "").toLowerCase().includes(q);
  });
});

const catLabel = computed(() => CATS.find((c) => c.key === cat.value)?.label ?? "全部预设");

/** 筛选器选项：每类带数量，全部预设给总数 */
const filterOptions = computed<FilterOption[]>(() => [
  { key: "all", label: "全部预设", count: props.presets.length },
  ...CATS.filter((c) => c.key !== "all").map((c) => ({
    key: c.key,
    label: c.label,
    count: props.presets.filter((p) => p.cat === c.key).length,
  })),
]);

/** 折叠时显示的那 4 张：保持原顺序；当前预设不在前 4 时顶到第 4 格 */
const visibleList = computed(() => {
  if (!collapsed.value) return filtered.value;
  const head = filtered.value.slice(0, VISIBLE_LIMIT);
  const cur = filtered.value.find((p) => p.id === props.curPresetId);
  if (cur && !head.some((p) => p.id === cur.id)) {
    if (head.length >= VISIBLE_LIMIT) head[VISIBLE_LIMIT - 1] = cur;
    else head.push(cur);
  }
  return head;
});

const hasRest = computed(() => filtered.value.length > VISIBLE_LIMIT);

function toggleFold() {
  collapsed.value = !collapsed.value;
}
</script>

<template>
  <div class="preset-list">
    <div class="preset-tabs">
      <button
        type="button"
        class="preset-tab"
        :class="{ 'is-active': tab === 'lib' }"
        @click="tab = 'lib'"
      >
        预设库
      </button>
      <button
        type="button"
        class="preset-tab"
        :class="{ 'is-active': tab === 'fav' }"
        @click="tab = 'fav'"
      >
        收藏
      </button>
      <span class="preset-tabs-tail" />
      <PresetFilterMenu
        :options="filterOptions"
        :model-value="cat"
        :total="presets.length"
        @update:model-value="cat = $event as CatKey"
      />
    </div>

    <div class="preset-search">
      <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor">
        <circle cx="7" cy="7" r="4.2" stroke-width="1.4" />
        <path d="M10.2 10.2 13.5 13.5" stroke-width="1.4" stroke-linecap="round" />
      </svg>
      <input v-model="query" class="preset-search-input" placeholder="搜索预设……" />
      <button v-if="query" type="button" class="preset-search-clear" @click="query = ''">✕</button>
    </div>

    <div class="preset-cat">
      <span>{{ catLabel }}</span>
      <span class="preset-cat-tail">{{ filtered.length }}</span>
    </div>

    <div class="preset-scroll wb-scroll">
      <p v-if="!filtered.length" class="wb-empty">
        {{ tab === "fav" ? "还没有收藏的预设" : "没有匹配的预设" }}
      </p>
      <button
        v-for="p in visibleList"
        :key="p.id"
        type="button"
        class="preset-card"
        :class="{ 'is-active': p.id === curPresetId }"
        @click="emit('select', p.id)"
      >
        <span class="preset-card-bar" />
        <span class="preset-card-body">
          <span class="preset-card-name wb-ellipsis">{{ shortName(p.name) }}</span>
          <span class="preset-card-desc wb-ellipsis">{{ p.desc }}</span>
        </span>
        <span
          class="preset-card-star"
          :class="{ 'is-on': favoriteIds.includes(p.id) }"
          :title="favoriteIds.includes(p.id) ? '取消收藏' : '收藏'"
          @click.stop="emit('toggle-favorite', p.id)"
          >★</span
        >
        <svg
          v-if="p.id === curPresetId"
          class="preset-card-check"
          viewBox="0 0 16 16"
          width="12"
          height="12"
          fill="none"
          stroke="currentColor"
        >
          <path d="M3.5 8.5 6.5 11.5 12.5 4.5" stroke-width="1.8" stroke-linecap="round" />
        </svg>
      </button>
    </div>

    <PresetFoldMenu v-if="hasRest" :collapsed="collapsed" @toggle="toggleFold" />

    <div class="preset-foot">
      <button type="button" class="wb-btn wb-btn-block" @click="emit('save-as')">
        <span style="font-size: 13px; line-height: 1">＋</span> 保存为预设
      </button>
    </div>
  </div>
</template>

<style scoped>
.preset-list {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}

.preset-tabs {
  display: flex;
  gap: 14px;
  padding: 0 12px;
  height: 32px;
  align-items: center;
  border-bottom: 1px solid var(--wb-border);
  flex-shrink: 0;
}

.preset-tab {
  position: relative;
  height: 32px;
  font-size: 11.5px;
  color: var(--wb-muted);
  transition: color 0.12s ease;
}

.preset-tab.is-active {
  color: var(--wb-text);
  font-weight: 600;
}

.preset-tab.is-active::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 2px;
  border-radius: 2px;
  background: var(--wb-accent);
}

.preset-tabs-tail {
  flex: 1;
}

.preset-search {
  position: relative;
  display: flex;
  align-items: center;
  gap: 5px;
  margin: 8px 10px 4px;
  padding: 0 8px;
  height: 26px;
  border: 1px solid var(--wb-border-strong);
  border-radius: var(--wb-r-md);
  color: var(--wb-faint);
  background: var(--wb-surface);
  transition: border-color 0.12s ease;
}

.preset-search:focus-within {
  border-color: var(--wb-accent);
  box-shadow: 0 0 0 3px rgb(110 86 248 / 10%);
}

.preset-search-input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  font-size: 11.5px;
  color: var(--wb-text);
}

.preset-search-input::placeholder {
  color: var(--wb-faint);
}

.preset-search-clear {
  color: var(--wb-faint);
  font-size: 10px;
}

.preset-search-clear:hover {
  color: var(--wb-text);
}

.preset-cat {
  display: flex;
  align-items: center;
  height: 24px;
  padding: 0 11px;
  font-size: 11px;
  color: var(--wb-text-2);
  flex-shrink: 0;
}

.preset-cat-tail {
  margin-left: auto;
  color: var(--wb-faint);
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}

.preset-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 2px 6px 6px;
}

.preset-card {
  position: relative;
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 7px 6px 7px 8px;
  border-radius: var(--wb-r-md);
  text-align: left;
  transition: background 0.12s ease;
}

.preset-card + .preset-card {
  margin-top: 2px;
}

.preset-card:hover {
  background: var(--wb-hover);
}

.preset-card.is-active {
  background: var(--wb-accent-weak);
}

.preset-card-bar {
  position: absolute;
  left: 0;
  top: 7px;
  bottom: 7px;
  width: 2px;
  border-radius: 2px;
  background: transparent;
}

.preset-card.is-active .preset-card-bar {
  background: var(--wb-accent);
}

.preset-card-body {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
  flex: 1;
}

.preset-card-name {
  font-size: 11.5px;
  font-weight: 500;
  color: var(--wb-text-2);
}

.preset-card.is-active .preset-card-name {
  color: var(--wb-accent);
  font-weight: 600;
}

.preset-card-desc {
  font-size: 10px;
  color: var(--wb-faint);
}

.preset-card-check {
  color: var(--wb-accent);
  flex-shrink: 0;
}

.preset-card-star {
  font-size: 10px;
  color: var(--wb-border-strong);
  opacity: 0;
  padding: 2px;
  transition:
    opacity 0.12s ease,
    color 0.12s ease;
}

.preset-card:hover .preset-card-star,
.preset-card-star.is-on {
  opacity: 1;
}

.preset-card-star.is-on {
  color: #f0a92f;
}

.preset-foot {
  padding: 6px 10px 8px;
  border-top: 1px solid var(--wb-border);
  flex-shrink: 0;
}
</style>

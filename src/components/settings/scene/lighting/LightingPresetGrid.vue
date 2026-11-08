<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import type { LightingPreset, PresetCategory } from "./types";

const props = withDefaults(
  defineProps<{
    presets: LightingPreset[];
    curPresetId: string;
    favoritePresetIds?: string[];
  }>(),
  {
    favoritePresetIds: () => [],
  },
);

const emit = defineEmits<{
  (e: "selectPreset", id: string): void;
  (e: "overrideCur"): void;
  (e: "revertCur"): void;
  (e: "toggleFavorite", id: string): void;
}>();

const activeCat = ref<PresetCategory>("all");

const categoryCounts = computed(() => {
  const counts: Record<PresetCategory, number> = {
    all: props.presets.length,
    favorite: props.presets.filter((p) => props.favoritePresetIds.includes(p.id)).length,
    daily: 0,
    atmosphere: 0,
    drama: 0,
    dark: 0,
  };
  props.presets.forEach((p) => {
    if (p.cat && counts[p.cat] !== undefined) counts[p.cat]++;
  });
  return counts;
});

const filteredPresets = computed(() => {
  if (activeCat.value === "all") return props.presets;
  if (activeCat.value === "favorite") {
    return props.presets.filter((p) => props.favoritePresetIds.includes(p.id));
  }
  return props.presets.filter((p) => p.cat === activeCat.value);
});

const activePreset = computed(() => props.presets.find((p) => p.id === props.curPresetId));
</script>

<template>
  <div class="flex flex-col gap-2.5">
    <!-- Header with Override / Revert / Quick selector -->
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2">
        <span class="text-xs font-bold text-slate-200">预设矩阵</span>
        <span class="text-[11px] text-slate-500">{{ filteredPresets.length }} 款</span>
      </div>

      <div class="flex items-center gap-1.5">
        <button
          type="button"
          @click="emit('overrideCur')"
          class="rounded border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 text-[11px] font-medium text-amber-300 transition-colors hover:bg-amber-500/20 active:scale-95"
          title="将当前面板全部修改覆盖保存至当前选中的预设"
        >
          覆盖当前
        </button>
        <button
          type="button"
          @click="emit('revertCur')"
          class="rounded border border-[#2e374b] bg-[#161a24] px-2 py-0.5 text-[11px] font-medium text-slate-400 transition-colors hover:bg-[#1e2433] hover:text-slate-200 active:scale-95"
          title="恢复当前选中预设的官方出厂参数"
        >
          重置内置
        </button>
      </div>
    </div>

    <!-- Quick select selector for fast navigation -->
    <div class="flex items-center gap-2">
      <span class="shrink-0 text-[11px] text-slate-400">快速切换:</span>
      <select
        :value="curPresetId"
        @change="emit('selectPreset', ($event.target as HTMLSelectElement).value)"
        class="h-7 w-full rounded border border-[#242b3d] bg-[#0d1017] px-2 text-xs text-slate-200 transition-colors outline-none focus:border-purple-500"
      >
        <option v-for="(p, idx) in presets" :key="p.id" :value="p.id">
          {{ String(idx + 1).padStart(2, "0") }} · {{ p.name.replace(/^\d{2}[·\.\s\-]\s*/, "") }}
          {{ p._overridden ? " [已调优]" : "" }}
        </option>
      </select>
    </div>

    <!-- Preset Grid (Strict vertical scrolling, 2 columns) -->
    <div
      class="grid max-h-[220px] grid-cols-2 gap-2 overflow-x-hidden overflow-y-auto pr-1"
      style="scrollbar-width: thin; scrollbar-color: #2b3347 transparent"
    >
      <div
        v-for="p in filteredPresets"
        :key="p.id"
        @click="emit('selectPreset', p.id)"
        :class="[
          'group flex min-w-0 cursor-pointer flex-col justify-between rounded-md border p-2 text-left transition-all',
          curPresetId === p.id
            ? 'border-purple-500 bg-purple-500/10 shadow-sm shadow-purple-500/20'
            : 'border-[#1f2533] bg-[#10131a] hover:border-[#2e374a] hover:bg-[#151923]',
        ]"
      >
        <div class="flex items-center justify-between gap-1">
          <div class="flex min-w-0 items-center gap-1.5">
            <button
              type="button"
              @click.stop="emit('toggleFavorite', p.id)"
              :class="[
                'text-xs transition-transform select-none hover:scale-125',
                favoritePresetIds.includes(p.id)
                  ? 'font-bold text-amber-400'
                  : 'text-slate-600 hover:text-amber-300',
              ]"
              :title="favoritePresetIds.includes(p.id) ? '取消收藏' : '加入收藏'"
            >
              ★
            </button>
            <div
              :class="[
                'truncate text-xs font-semibold',
                curPresetId === p.id ? 'text-purple-300' : 'text-slate-200 group-hover:text-white',
              ]"
            >
              {{ p.name }}
            </div>
          </div>
          <span
            v-if="p._overridden"
            class="shrink-0 rounded border border-amber-500/40 bg-amber-500/20 px-1 py-0.5 text-[9px] font-semibold text-amber-300"
          >
            {{ $t("settings.sceneEdit.lightingWorkbench.presets.optimizedBadge") }}
          </span>
        </div>
        <div class="mt-1 line-clamp-2 text-[10px] text-slate-400">
          {{ p.desc }}
        </div>
      </div>

      <div
        v-if="filteredPresets.length === 0"
        class="col-span-2 py-8 text-center text-xs text-slate-500"
      >
        {{
          activeCat === "favorite"
            ? $t("settings.sceneEdit.lightingWorkbench.presets.emptyFavorite")
            : "暂无对应分类预设"
        }}
      </div>
    </div>

    <!-- Category filter chips placed UNDER the grid -->
    <div class="flex flex-wrap items-center gap-1.5 pt-1">
      <button
        type="button"
        @click="activeCat = 'all'"
        :class="[
          'rounded-full px-2 py-0.5 text-[10px] font-medium transition-colors',
          activeCat === 'all'
            ? 'bg-purple-600/90 text-white'
            : 'bg-[#161a24] text-slate-400 hover:bg-[#202636] hover:text-slate-200',
        ]"
      >
        全部 ({{ categoryCounts.all }})
      </button>
      <button
        type="button"
        @click="activeCat = 'favorite'"
        :class="[
          'rounded-full px-2 py-0.5 text-[10px] font-medium transition-colors',
          activeCat === 'favorite'
            ? 'bg-amber-600 text-white shadow-sm'
            : 'bg-[#161a24] text-amber-400/70 hover:bg-[#202636] hover:text-amber-300',
        ]"
      >
        {{ $t("settings.sceneEdit.lightingWorkbench.presets.categories.favorite") }} ({{
          categoryCounts.favorite
        }})
      </button>
      <button
        type="button"
        @click="activeCat = 'daily'"
        :class="[
          'rounded-full px-2 py-0.5 text-[10px] font-medium transition-colors',
          activeCat === 'daily'
            ? 'bg-purple-600/90 text-white'
            : 'bg-[#161a24] text-slate-400 hover:bg-[#202636] hover:text-slate-200',
        ]"
      >
        日常 ({{ categoryCounts.daily }})
      </button>
      <button
        type="button"
        @click="activeCat = 'atmosphere'"
        :class="[
          'rounded-full px-2 py-0.5 text-[10px] font-medium transition-colors',
          activeCat === 'atmosphere'
            ? 'bg-purple-600/90 text-white'
            : 'bg-[#161a24] text-slate-400 hover:bg-[#202636] hover:text-slate-200',
        ]"
      >
        氛围 ({{ categoryCounts.atmosphere }})
      </button>
      <button
        type="button"
        @click="activeCat = 'drama'"
        :class="[
          'rounded-full px-2 py-0.5 text-[10px] font-medium transition-colors',
          activeCat === 'drama'
            ? 'bg-purple-600/90 text-white'
            : 'bg-[#161a24] text-slate-400 hover:bg-[#202636] hover:text-slate-200',
        ]"
      >
        戏剧 ({{ categoryCounts.drama }})
      </button>
      <button
        type="button"
        @click="activeCat = 'dark'"
        :class="[
          'rounded-full px-2 py-0.5 text-[10px] font-medium transition-colors',
          activeCat === 'dark'
            ? 'bg-purple-600/90 text-white'
            : 'bg-[#161a24] text-slate-400 hover:bg-[#202636] hover:text-slate-200',
        ]"
      >
        暗黑 ({{ categoryCounts.dark }})
      </button>
    </div>
  </div>
</template>

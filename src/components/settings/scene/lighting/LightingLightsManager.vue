<script setup lang="ts">
import type { LightSource, LightType } from "@/api/services/scene";

const props = defineProps<{
  lights: LightSource[];
  selectedLightIndex: number;
  selectedLight: LightSource | null;
}>();

const emit = defineEmits<{
  (e: "update:selectedLightIndex", index: number): void;
  (e: "addLight", type: LightType): void;
  (e: "removeLight", index: number): void;
  (e: "toggleKeyLight", index: number): void;
  (e: "moveLight", index: number, direction: "up" | "down"): void;
}>();

const TYPE_NAMES: Record<LightType, string> = {
  point: "点光",
  directional: "平行光",
  spot: "聚光灯",
  ambient: "环境光",
};
</script>

<template>
  <div class="flex flex-col gap-3">
    <!-- Header & Light counter -->
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-1.5">
        <span class="text-xs font-bold text-slate-200">光源管理</span>
        <span class="text-[11px] text-slate-400">({{ lights.length }}/8 盏 · 支持优先级排序)</span>
      </div>
    </div>

    <!-- Add Light buttons row -->
    <div class="grid grid-cols-4 gap-1.5">
      <button
        type="button"
        @click="emit('addLight', 'point')"
        :disabled="lights.length >= 8"
        class="rounded border border-[#242b3d] bg-[#121620] py-1 text-[11px] font-medium text-slate-300 transition-colors hover:border-purple-500/50 hover:bg-[#181d2a] hover:text-white disabled:opacity-40"
      >
        + 点光源
      </button>
      <button
        type="button"
        @click="emit('addLight', 'directional')"
        :disabled="lights.length >= 8"
        class="rounded border border-[#242b3d] bg-[#121620] py-1 text-[11px] font-medium text-slate-300 transition-colors hover:border-purple-500/50 hover:bg-[#181d2a] hover:text-white disabled:opacity-40"
      >
        + 平行光
      </button>
      <button
        type="button"
        @click="emit('addLight', 'spot')"
        :disabled="lights.length >= 8"
        class="rounded border border-[#242b3d] bg-[#121620] py-1 text-[11px] font-medium text-slate-300 transition-colors hover:border-purple-500/50 hover:bg-[#181d2a] hover:text-white disabled:opacity-40"
      >
        + 聚光灯
      </button>
      <button
        type="button"
        @click="emit('addLight', 'ambient')"
        :disabled="lights.length >= 8"
        class="rounded border border-[#242b3d] bg-[#121620] py-1 text-[11px] font-medium text-slate-300 transition-colors hover:border-purple-500/50 hover:bg-[#181d2a] hover:text-white disabled:opacity-40"
      >
        + 环境光
      </button>
    </div>

    <!-- Lights List -->
    <div
      class="flex max-h-[160px] flex-col gap-1 overflow-y-auto pr-1"
      style="scrollbar-width: thin; scrollbar-color: #2b3347 transparent"
    >
      <div
        v-for="(l, idx) in lights"
        :key="l.id"
        @click="emit('update:selectedLightIndex', idx)"
        :class="[
          'flex cursor-pointer items-center justify-between rounded border px-2 py-1 text-xs transition-colors',
          selectedLightIndex === idx
            ? 'border-purple-500 bg-purple-500/15 text-white'
            : 'border-[#1b202c] bg-[#0f1218] text-slate-300 hover:border-[#2b3347] hover:bg-[#141822]',
        ]"
      >
        <div class="flex items-center gap-2 overflow-hidden">
          <!-- Light color dot -->
          <span
            class="h-2.5 w-2.5 shrink-0 rounded-full border border-black/40"
            :style="{ backgroundColor: l.color }"
          ></span>

          <!-- Light index & name -->
          <span class="shrink-0 font-mono text-[10px] text-slate-500">#{{ idx + 1 }}</span>
          <span class="truncate text-xs font-medium">{{ l.name }}</span>
          <span class="shrink-0 text-[9px] text-slate-500 uppercase">
            {{ TYPE_NAMES[l.type] }}
          </span>
        </div>

        <div class="flex shrink-0 items-center gap-1.5" @click.stop>
          <!-- Key light toggle -->
          <button
            type="button"
            @click="emit('toggleKeyLight', idx)"
            :class="[
              'text-xs transition-colors',
              l.is_key ? 'text-amber-400' : 'text-slate-600 hover:text-slate-400',
            ]"
            title="设为主光"
          >
            ★
          </button>

          <!-- Move priority up / down -->
          <button
            type="button"
            @click="emit('moveLight', idx, 'up')"
            :disabled="idx === 0"
            class="text-[10px] text-slate-500 hover:text-slate-200 disabled:opacity-20"
          >
            ▲
          </button>
          <button
            type="button"
            @click="emit('moveLight', idx, 'down')"
            :disabled="idx === lights.length - 1"
            class="text-[10px] text-slate-500 hover:text-slate-200 disabled:opacity-20"
          >
            ▼
          </button>

          <!-- Delete -->
          <button
            type="button"
            @click="emit('removeLight', idx)"
            :disabled="lights.length <= 1"
            class="text-xs text-slate-600 hover:text-rose-400 disabled:opacity-20"
            title="删除光源"
          >
            ✕
          </button>
        </div>
      </div>
    </div>

    <!-- Selected Light Parameter Inspector -->
    <div
      v-if="selectedLight"
      class="flex flex-col gap-2 rounded-lg border border-[#212736] bg-[#0c0f16] p-3 text-xs"
    >
      <div class="flex items-center justify-between border-b border-[#1b202c] pb-2">
        <span class="font-semibold text-slate-300"> 选定光源: {{ selectedLight.name }} </span>
        <label class="flex cursor-pointer items-center gap-1.5 text-[11px] text-slate-400">
          <input type="checkbox" v-model="selectedLight.is_key" class="rounded accent-amber-500" />
          主光
        </label>
      </div>

      <!-- Name & Color -->
      <div class="grid grid-cols-2 gap-2">
        <div class="flex flex-col gap-1">
          <span class="text-[11px] text-slate-400">名称</span>
          <input
            type="text"
            v-model="selectedLight.name"
            class="h-7 rounded border border-[#242b3d] bg-[#141824] px-2 text-xs text-slate-200 outline-none focus:border-purple-500"
          />
        </div>
        <div class="flex flex-col gap-1">
          <span class="text-[11px] text-slate-400">颜色</span>
          <div class="flex items-center gap-2">
            <input
              type="color"
              v-model="selectedLight.color"
              class="h-7 w-8 cursor-pointer rounded border-0 bg-transparent p-0"
            />
            <input
              type="text"
              v-model="selectedLight.color"
              class="h-7 w-full rounded border border-[#242b3d] bg-[#141824] px-2 font-mono text-[11px] text-slate-200 uppercase outline-none focus:border-purple-500"
            />
          </div>
        </div>
      </div>

      <!-- Intensity -->
      <div class="flex flex-col gap-1">
        <div class="flex justify-between text-[11px] text-slate-400">
          <span>{{ $t("settings.sceneEdit.lightingWorkbench.lights.intensity") }}</span>
          <span class="font-mono text-purple-300">{{ selectedLight.intensity.toFixed(2) }}</span>
        </div>
        <input
          type="range"
          min="0"
          max="3.0"
          step="0.01"
          v-model.number="selectedLight.intensity"
          class="cursor-pointer accent-purple-500"
        />
      </div>

      <!-- Point / Spot Position and Geometry -->
      <template v-if="selectedLight.type === 'point' || selectedLight.type === 'spot'">
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>{{ $t("settings.sceneEdit.lightingWorkbench.lights.radius") }}</span>
            <span class="font-mono text-slate-300">{{ selectedLight.radius.toFixed(2) }}</span>
          </div>
          <input
            type="range"
            min="0.1"
            max="2.5"
            step="0.01"
            v-model.number="selectedLight.radius"
            class="cursor-pointer accent-purple-500"
          />
        </div>

        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>边缘软硬 (Softness)</span>
            <span class="font-mono text-slate-300">{{ selectedLight.softness.toFixed(2) }}</span>
          </div>
          <input
            type="range"
            min="0.0"
            max="1.0"
            step="0.01"
            v-model.number="selectedLight.softness"
            class="cursor-pointer accent-purple-500"
          />
        </div>

        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>{{ $t("settings.sceneEdit.lightingWorkbench.lights.depthZ") }}</span>
            <span class="font-mono text-slate-300">{{ selectedLight.z.toFixed(2) }}</span>
          </div>
          <input
            type="range"
            min="0.0"
            max="1.0"
            step="0.01"
            v-model.number="selectedLight.z"
            class="cursor-pointer accent-purple-500"
          />
        </div>
      </template>

      <!-- Directional / Spot Azimuth and Elevation -->
      <template v-if="selectedLight.type === 'directional' || selectedLight.type === 'spot'">
        <div class="grid grid-cols-2 gap-2 pt-1">
          <div class="flex flex-col gap-1">
            <div class="flex justify-between text-[11px] text-slate-400">
              <span>{{ $t("settings.sceneEdit.lightingWorkbench.lights.azimuth") }}</span>
              <span class="font-mono text-slate-300">{{ Math.round(selectedLight.azimuth) }}°</span>
            </div>
            <input
              type="range"
              min="0"
              max="360"
              step="1"
              v-model.number="selectedLight.azimuth"
              class="cursor-pointer accent-purple-500"
            />
          </div>
          <div class="flex flex-col gap-1">
            <div class="flex justify-between text-[11px] text-slate-400">
              <span>{{ $t("settings.sceneEdit.lightingWorkbench.lights.elevation") }}</span>
              <span class="font-mono text-slate-300"
                >{{ Math.round(selectedLight.elevation) }}°</span
              >
            </div>
            <input
              type="range"
              min="0"
              max="90"
              step="1"
              v-model.number="selectedLight.elevation"
              class="cursor-pointer accent-purple-500"
            />
          </div>
        </div>
      </template>

      <!-- Spot Cone and Penumbra -->
      <template v-if="selectedLight.type === 'spot'">
        <div class="grid grid-cols-2 gap-2 pt-1">
          <div class="flex flex-col gap-1">
            <div class="flex justify-between text-[11px] text-slate-400">
              <span>锥角</span>
              <span class="font-mono text-slate-300">{{ Math.round(selectedLight.cone) }}°</span>
            </div>
            <input
              type="range"
              min="5"
              max="90"
              step="1"
              v-model.number="selectedLight.cone"
              class="cursor-pointer accent-purple-500"
            />
          </div>
          <div class="flex flex-col gap-1">
            <div class="flex justify-between text-[11px] text-slate-400">
              <span>半影过渡</span>
              <span class="font-mono text-slate-300">{{ selectedLight.penumbra.toFixed(2) }}</span>
            </div>
            <input
              type="range"
              min="0"
              max="1.0"
              step="0.01"
              v-model.number="selectedLight.penumbra"
              class="cursor-pointer accent-purple-500"
            />
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

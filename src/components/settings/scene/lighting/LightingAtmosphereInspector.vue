<script setup lang="ts">
import type { LightingCompositingParams, LightingGlobalParams } from "@/api/services/scene";
import { DEFAULT_COMPOSITING, DEFAULT_GLOBAL } from "./types";

const props = defineProps<{
  compositingParams: LightingCompositingParams;
  globalParams: LightingGlobalParams;
}>();

const emit = defineEmits<{
  (e: "resetCompositing"): void;
  (e: "resetGlobal"): void;
}>();

function resetCompositing() {
  Object.assign(props.compositingParams, DEFAULT_COMPOSITING);
  emit("resetCompositing");
}

function resetGlobal() {
  Object.assign(props.globalParams, DEFAULT_GLOBAL);
  emit("resetGlobal");
}
</script>

<template>
  <div class="flex flex-col gap-3">
    <!-- 2D Compositing -->
    <div class="flex flex-col gap-2.5 rounded-lg border border-[#212736] bg-[#0c0f16] p-3 text-xs">
      <div class="flex items-center justify-between border-b border-[#1b202c] pb-2">
        <span class="font-bold text-slate-200">边缘与空气感 (2D 合成)</span>
        <button
          type="button"
          @click="resetCompositing"
          class="text-[11px] text-slate-400 transition-colors hover:text-purple-300"
        >
          重置本组
        </button>
      </div>

      <div class="flex flex-col gap-2">
        <!-- Light wrap -->
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>{{ $t("settings.sceneEdit.lightingWorkbench.compositing.lightWrap") }}</span>
            <span class="font-mono text-purple-300">{{
              compositingParams.light_wrap.toFixed(2)
            }}</span>
          </div>
          <input
            type="range"
            min="0.0"
            max="1.5"
            step="0.01"
            v-model.number="compositingParams.light_wrap"
            class="cursor-pointer accent-purple-500"
          />
        </div>

        <!-- Rim width -->
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>边缘光宽度 (Rim Width)</span>
            <span class="font-mono text-purple-300"
              >{{ compositingParams.rim_width.toFixed(1) }}px</span
            >
          </div>
          <input
            type="range"
            min="0.0"
            max="30.0"
            step="0.1"
            v-model.number="compositingParams.rim_width"
            class="cursor-pointer accent-purple-500"
          />
        </div>

        <!-- Bloom intensity -->
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>晕光泛光 (Bloom Intensity)</span>
            <span class="font-mono text-purple-300">{{
              compositingParams.bloom_int.toFixed(2)
            }}</span>
          </div>
          <input
            type="range"
            min="0.0"
            max="2.0"
            step="0.01"
            v-model.number="compositingParams.bloom_int"
            class="cursor-pointer accent-purple-500"
          />
        </div>

        <!-- Bloom threshold -->
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>泛光阈值 (Threshold)</span>
            <span class="font-mono text-purple-300">{{
              compositingParams.bloom_thresh.toFixed(2)
            }}</span>
          </div>
          <input
            type="range"
            min="0.2"
            max="1.0"
            step="0.01"
            v-model.number="compositingParams.bloom_thresh"
            class="cursor-pointer accent-purple-500"
          />
        </div>
      </div>
    </div>

    <!-- Global Tone -->
    <div class="flex flex-col gap-2.5 rounded-lg border border-[#212736] bg-[#0c0f16] p-3 text-xs">
      <div class="flex items-center justify-between border-b border-[#1b202c] pb-2">
        <span class="font-bold text-slate-200">全局色彩调性 (Tone)</span>
        <button
          type="button"
          @click="resetGlobal"
          class="text-[11px] text-slate-400 transition-colors hover:text-purple-300"
        >
          重置本组
        </button>
      </div>

      <div class="flex flex-col gap-2">
        <!-- Master -->
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>主光照强度 (Master)</span>
            <span class="font-mono text-purple-300">{{ globalParams.master.toFixed(2) }}</span>
          </div>
          <input
            type="range"
            min="0.0"
            max="2.0"
            step="0.01"
            v-model.number="globalParams.master"
            class="cursor-pointer accent-purple-500"
          />
        </div>

        <!-- Ambient blend -->
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>环境染色比例 (Ambient Blend)</span>
            <span class="font-mono text-purple-300">{{
              globalParams.ambient_blend.toFixed(2)
            }}</span>
          </div>
          <input
            type="range"
            min="0.0"
            max="1.0"
            step="0.01"
            v-model.number="globalParams.ambient_blend"
            class="cursor-pointer accent-purple-500"
          />
        </div>

        <!-- Albedo protect -->
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>高亮不过曝保护 (Albedo Protect)</span>
            <span class="font-mono text-purple-300">{{
              globalParams.albedo_protect.toFixed(2)
            }}</span>
          </div>
          <input
            type="range"
            min="0.0"
            max="1.0"
            step="0.01"
            v-model.number="globalParams.albedo_protect"
            class="cursor-pointer accent-purple-500"
          />
        </div>

        <!-- Lift -->
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>暗部抬升 (Lift)</span>
            <span class="font-mono text-purple-300">{{ globalParams.lift.toFixed(2) }}</span>
          </div>
          <input
            type="range"
            min="0.0"
            max="0.5"
            step="0.01"
            v-model.number="globalParams.lift"
            class="cursor-pointer accent-purple-500"
          />
        </div>

        <!-- Soft knee -->
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>软膝色调映射 (Soft Knee)</span>
            <span class="font-mono text-purple-300">{{ globalParams.soft_knee.toFixed(2) }}</span>
          </div>
          <input
            type="range"
            min="0.2"
            max="1.0"
            step="0.01"
            v-model.number="globalParams.soft_knee"
            class="cursor-pointer accent-purple-500"
          />
        </div>

        <!-- Mono -->
        <div class="flex flex-col gap-1">
          <div class="flex justify-between text-[11px] text-slate-400">
            <span>去色 / 单色调 (Mono)</span>
            <span class="font-mono text-purple-300">{{ globalParams.mono.toFixed(2) }}</span>
          </div>
          <input
            type="range"
            min="0.0"
            max="1.0"
            step="0.01"
            v-model.number="globalParams.mono"
            class="cursor-pointer accent-purple-500"
          />
        </div>
      </div>
    </div>
  </div>
</template>

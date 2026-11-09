<script setup lang="ts">
import type { LightingMaterialParams } from "@/api/services/scene";
import { DEFAULT_MATERIAL } from "./types";

const props = defineProps<{
  materialParams: LightingMaterialParams;
}>();

const emit = defineEmits<{
  (e: "reset"): void;
}>();

function resetGroup() {
  Object.assign(props.materialParams, DEFAULT_MATERIAL);
  emit("reset");
}
</script>

<template>
  <div class="flex flex-col gap-3 rounded-lg border border-[#212736] bg-[#0c0f16] p-3 text-xs">
    <!-- Header with Reset button -->
    <div class="flex items-center justify-between border-b border-[#1b202c] pb-2">
      <div class="flex items-center gap-1.5">
        <span class="font-bold text-slate-200">2.5D 体积材质 (Normal & Depth)</span>
      </div>
      <button
        type="button"
        @click="resetGroup"
        class="text-[11px] text-slate-400 transition-colors hover:text-purple-300"
      >
        重置本组
      </button>
    </div>

    <!-- 3 Texture Slots Preview -->
    <div class="grid grid-cols-3 gap-2">
      <div class="flex flex-col items-center rounded border border-[#1f2636] bg-[#090b10] p-1.5">
        <div class="flex h-14 w-full items-center justify-center rounded bg-slate-900/60 text-lg">
          🖼️
        </div>
        <span class="mt-1 text-[10px] text-slate-400">原图 Diffuse</span>
      </div>
      <div class="flex flex-col items-center rounded border border-[#1f2636] bg-[#090b10] p-1.5">
        <div class="flex h-14 w-full items-center justify-center rounded bg-indigo-950/40 text-lg">
          🟣
        </div>
        <span class="mt-1 text-[10px] text-indigo-300">{{
          $t("settings.sceneEdit.lightingWorkbench.material.normalMap")
        }}</span>
      </div>
      <div class="flex flex-col items-center rounded border border-[#1f2636] bg-[#090b10] p-1.5">
        <div class="flex h-14 w-full items-center justify-center rounded bg-slate-800/40 text-lg">
          ⚪
        </div>
        <span class="mt-1 text-[10px] text-slate-300">{{
          $t("settings.sceneEdit.lightingWorkbench.material.depthMap")
        }}</span>
      </div>
    </div>

    <!-- Sliders -->
    <div class="flex flex-col gap-2 pt-1">
      <!-- Normal strength -->
      <div class="flex flex-col gap-1">
        <div class="flex justify-between text-[11px] text-slate-400">
          <span>{{ $t("settings.sceneEdit.lightingWorkbench.material.normalStrength") }}</span>
          <span class="font-mono text-purple-300">{{
            materialParams.normal_strength.toFixed(2)
          }}</span>
        </div>
        <input
          type="range"
          min="0.0"
          max="2.0"
          step="0.01"
          v-model.number="materialParams.normal_strength"
          class="cursor-pointer accent-purple-500"
        />
      </div>

      <!-- Cel threshold -->
      <div class="flex flex-col gap-1">
        <div class="flex justify-between text-[11px] text-slate-400">
          <span>交界阴影阈值</span>
          <span class="font-mono text-purple-300">{{ materialParams.cel_thresh.toFixed(2) }}</span>
        </div>
        <input
          type="range"
          min="0.0"
          max="1.0"
          step="0.01"
          v-model.number="materialParams.cel_thresh"
          class="cursor-pointer accent-purple-500"
        />
      </div>

      <!-- Cel softness -->
      <div class="flex flex-col gap-1">
        <div class="flex justify-between text-[11px] text-slate-400">
          <span>{{ $t("settings.sceneEdit.lightingWorkbench.material.celSoftness") }}</span>
          <span class="font-mono text-purple-300">{{
            materialParams.cel_softness.toFixed(2)
          }}</span>
        </div>
        <input
          type="range"
          min="0.0"
          max="0.5"
          step="0.01"
          v-model.number="materialParams.cel_softness"
          class="cursor-pointer accent-purple-500"
        />
      </div>

      <!-- Specular intensity -->
      <div class="flex flex-col gap-1">
        <div class="flex justify-between text-[11px] text-slate-400">
          <span>{{ $t("settings.sceneEdit.lightingWorkbench.material.specularIntensity") }}</span>
          <span class="font-mono text-purple-300">{{ materialParams.spec_int.toFixed(2) }}</span>
        </div>
        <input
          type="range"
          min="0.0"
          max="1.0"
          step="0.01"
          v-model.number="materialParams.spec_int"
          class="cursor-pointer accent-purple-500"
        />
      </div>

      <!-- Specular glossiness -->
      <div class="flex flex-col gap-1">
        <div class="flex justify-between text-[11px] text-slate-400">
          <span>{{ $t("settings.sceneEdit.lightingWorkbench.material.specularShininess") }}</span>
          <span class="font-mono text-purple-300">{{ Math.round(materialParams.spec_gloss) }}</span>
        </div>
        <input
          type="range"
          min="2"
          max="128"
          step="1"
          v-model.number="materialParams.spec_gloss"
          class="cursor-pointer accent-purple-500"
        />
      </div>

      <!-- Self shadow intensity -->
      <div class="flex flex-col gap-1">
        <div class="flex justify-between text-[11px] text-slate-400">
          <span>深度自阴影</span>
          <span class="font-mono text-purple-300">{{
            materialParams.self_shadow_int.toFixed(2)
          }}</span>
        </div>
        <input
          type="range"
          min="0.0"
          max="1.0"
          step="0.01"
          v-model.number="materialParams.self_shadow_int"
          class="cursor-pointer accent-purple-500"
        />
      </div>

      <!-- 两个总开关：原来只存在于数据结构里，界面没有任何入口 -->
      <div class="flex items-center justify-between border-t border-[#1b202c] pt-2 text-[11px]">
        <div class="flex flex-col">
          <span class="text-slate-300">启用伪法线</span>
          <span class="text-[10px] text-slate-500">关闭后退化为平面着色，光会变平但更省</span>
        </div>
        <input
          type="checkbox"
          v-model="materialParams.use_normal"
          class="h-4 w-4 cursor-pointer accent-purple-500"
        />
      </div>

      <div class="flex items-center justify-between text-[11px]">
        <div class="flex flex-col">
          <span class="text-slate-300">启用伪深度</span>
          <span class="text-[10px] text-slate-500">从立绘 alpha 推导，关闭后自阴影失效</span>
        </div>
        <input
          type="checkbox"
          v-model="materialParams.use_depth"
          class="h-4 w-4 cursor-pointer accent-purple-500"
        />
      </div>
    </div>
  </div>
</template>

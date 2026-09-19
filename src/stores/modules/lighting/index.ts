/**
 * 光影生效状态。
 *
 * 三条来源写到这里：剧本 `lighting` 事件、`lighting_apply` 工具、设置面板。
 * 渲染层（背景 / 舞台 / 立绘三个组件）只读 `active`，不再各自判断优先级——
 * 原先每个组件都直接读 `gameStore.currentScene.lighting`，谁都能改灯光却没有
 * 统一的裁决点，剧本一改就会和场景设置打架。
 *
 * 生效优先级：总开关关 → 完全没有光影；否则
 * 运行时覆盖（剧本/工具）→ 全局预设（设置面板）→ 场景自带灯光。
 */
import { defineStore } from "pinia";

import { listLightingPresets } from "@/api/services/lighting";
import type { LightingChangePayload, LightingPreset } from "@/api/services/lighting";
import type { LightingParams } from "@/api/services/scene";
import { planLighting } from "@/utils/lighting";
import type { LightingPlan } from "@/utils/lighting";
import type { LightingSettings } from "../settings";
import { useGameStore } from "../game";
import { useSettingsStore } from "../settings";

interface OverrideState {
  preset: string | null;
  params: LightingParams;
  source: string;
}

/** 子开关只负责「压掉」参数里自带的启用位，不会反向打开场景没启用的效果。 */
function maskFeatures(params: LightingParams, on: LightingSettings): LightingParams {
  return {
    ...params,
    overlay_enabled: params.overlay_enabled && on.overlayEnabled,
    directional_enabled: params.directional_enabled && on.directionalEnabled,
    bloom_enabled: params.bloom_enabled && on.bloomEnabled,
    vignette_enabled: params.vignette_enabled && on.vignetteEnabled,
    grade_enabled: params.grade_enabled && on.gradeEnabled,
    character: { ...params.character, rim_enabled: params.character?.rim_enabled && on.rimEnabled },
    // 低性能模式只砍持续动画，静态光影照常保留
    breathing_enabled: params.breathing_enabled && on.breathingEnabled && !on.lowPerfMode,
  };
}

export const useLightingStore = defineStore("lighting", {
  state: () => ({
    /** 剧本事件 / LLM 工具写入的运行时覆盖；null = 跟随场景 */
    override: null as OverrideState | null,
    presets: [] as LightingPreset[],
    presetsLoaded: false,
  }),

  getters: {
    /** 设置面板选定的全局预设参数；空串或未知 id 视为「跟随场景」。 */
    globalParams(state): LightingParams | null {
      const id = useSettingsStore().lighting.globalPreset;
      if (!id) return null;
      return state.presets.find((p) => p.id === id)?.params ?? null;
    },

    /** 未经开关屏蔽的生效参数来源。 */
    baseParams(state): LightingParams | null {
      if (state.override) return state.override.params;
      return this.globalParams ?? useGameStore().currentScene?.lighting ?? null;
    },

    /** 渲染层唯一该读的东西：已应用总开关与子开关的最终参数。 */
    active(): LightingParams | null {
      const s = useSettingsStore();
      if (!s.lighting.masterEnabled) return null;
      const base = this.baseParams;
      return base ? maskFeatures(base, s.lighting) : null;
    },

    /** 参数 → 各层 CSS，一次算好给三个组件共用。 */
    plan(): LightingPlan {
      return planLighting(this.active);
    },

    /** 当前生效的预设 id，供面板高亮与状态回显（空串 = 跟随场景）。 */
    activePresetId(state): string {
      const fromOverride = state.override?.preset;
      if (fromOverride) return fromOverride;
      return state.override ? "" : useSettingsStore().lighting.globalPreset;
    },

    /** 谁在控制灯光：`scene` / `global` / `script` / `tool` / `panel`。 */
    activeSource(state): string {
      return state.override?.source ?? (this.globalParams ? "global" : "scene");
    },
  },

  actions: {
    /** 预设表懒加载：面板和全局预设解析都要用它。 */
    async ensurePresets() {
      if (this.presetsLoaded) return;
      this.presetsLoaded = true;
      try {
        this.presets = await listLightingPresets();
      } catch (e) {
        this.presetsLoaded = false;
        console.warn("[Lighting] 预设列表加载失败:", e);
      }
    },

    /** 收到 `lighting:change` / `script:lighting` 广播。params 为 null 即清回跟随场景。 */
    applyPayload(payload: LightingChangePayload) {
      this.override = payload.params
        ? {
            preset: payload.preset ?? null,
            params: payload.params,
            source: payload.source ?? "script",
          }
        : null;
    },

    /** 剧本结束 / 场景重置时调用：回到「跟随场景」。 */
    clearOverride() {
      this.override = null;
    },
  },
});

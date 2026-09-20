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
 *
 * 后端看不见这份裁决结果：全局预设存在 localStorage 里，渲染层又在覆盖之后。
 * 所以 `startActiveReporting` 会把最终生效的预设回传后端，`lighting_get` 才答得
 * 出「现在到底在打什么灯」——否则 AI 读到一个永远为空的可观测量，就会误判成
 * 「没打灯」或沿用上一次的结果，跳过用户要求的切换。
 */
import { defineStore } from "pinia";
import { watch } from "vue";

import {
  deleteLightingPreset,
  listLightingPresets,
  reportLightingActive,
} from "@/api/services/lighting";
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

/** 上报器只需要一份，重复调用（热更新、多入口）不再叠 watcher。 */
let reportingStarted = false;

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

    /** 自建预设增删后强制重取，让面板卡片和 AI 的工具返回值立刻一致。 */
    async refreshPresets() {
      this.presets = await listLightingPresets();
      this.presetsLoaded = true;
    },

    /**
     * 删掉一个自建预设。
     *
     * 如果它正被当成全局预设用，必须一并清成「跟随场景」：留着失效 id 会让
     * `activePresetId` 报出一个不存在的灯，上报给后端的就是假状态。
     */
    async removePreset(id: string) {
      await deleteLightingPreset(id);
      const settings = useSettingsStore();
      if (settings.lighting.globalPreset === id) settings.updateLighting({ globalPreset: "" });
      await this.refreshPresets();
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

    /**
     * 开始把生效光影回传后端。只在主窗口的监听初始化里调一次。
     *
     * 用 watch 而不是在每个改动点手动上报：裁决结果就这三个 getter，靠人工追
     * 状态早晚会漏一处——而漏掉的那一处正是这次「AI 说已经调好了」的来源。
     */
    startActiveReporting() {
      if (reportingStarted) return;
      reportingStarted = true;
      const settings = useSettingsStore();
      watch(
        () => [settings.lighting.masterEnabled, this.activePresetId, this.activeSource] as const,
        ([masterOn, preset, source]) => {
          void reportLightingActive({
            preset: masterOn && preset ? preset : null,
            source: masterOn ? source : "off",
          }).catch((e) => console.warn("[Lighting] 生效状态上报失败:", e));
        },
        { immediate: true }
      );
    },
  },
});

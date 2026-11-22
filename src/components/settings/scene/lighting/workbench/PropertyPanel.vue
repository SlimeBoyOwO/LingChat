<script setup lang="ts">
/**
 * 右栏：属性 / 历史记录。
 *
 * 稿上把沙箱的 7 个 section 收成三组常驻 + 两个折叠入口：
 * 光源参数 → 边缘与空气感（+ 更多参数）→ 体积光材质（+ 高级参数）。
 * 所有改动都只 emit intent，由 Shell 交给 store。
 */
import { computed } from "vue";
import type {
  LightSource,
  LightingCompositingParams,
  LightingGlobalParams,
  LightingMaterialParams,
} from "@/api/services/scene";
import type { RightTab } from "./useWorkbenchUi";
import LightParamsPanel from "./LightParamsPanel.vue";
import ParamsGroup from "./ParamsGroup.vue";
import ParamSlider from "./ParamSlider.vue";
import ParamToggle from "./ParamToggle.vue";
import ParamColor from "./ParamColor.vue";
import MapPreviewGrid from "./MapPreviewGrid.vue";
import HistoryPanel from "./HistoryPanel.vue";

const props = defineProps<{
  rightTab: RightTab;
  light: LightSource | null;
  baselineLight: LightSource | null;
  globalParams: LightingGlobalParams;
  compositingParams: LightingCompositingParams;
  materialParams: LightingMaterialParams;
  characterSrc?: string | null;
  normalSrc?: string | null;
  depthSrc?: string | null;
  expandAir: boolean;
  expandMaterial: boolean;
  presetName?: string;
  isFavorite: boolean;
  canUndo: boolean;
  canRedo: boolean;
  isDirty: boolean;
}>();

const emit = defineEmits<{
  (e: "update:rightTab", tab: RightTab): void;
  (e: "update:expandAir", v: boolean): void;
  (e: "update:expandMaterial", v: boolean): void;
  (e: "patch-light", patch: Partial<LightSource>): void;
  (e: "patch-global", patch: Partial<LightingGlobalParams>): void;
  (e: "patch-compositing", patch: Partial<LightingCompositingParams>): void;
  (e: "patch-material", patch: Partial<LightingMaterialParams>): void;
  (e: "update:normalSrc", v: string | null): void;
  (e: "update:depthSrc", v: string | null): void;
  (e: "reset-compositing"): void;
  (e: "reset-material"): void;
  (e: "reset-global"): void;
  (e: "toggle-favorite"): void;
  (e: "undo"): void;
  (e: "redo"): void;
  (e: "revert-preset"): void;
}>();

const TABS: { key: RightTab; label: string }[] = [
  { key: "props", label: "属性" },
  { key: "history", label: "历史记录" },
];

/** 预设里同位置的灯，作为双击复位的基准 */
const baseline = computed(() => props.baselineLight);

function globalNum(key: keyof LightingGlobalParams): number {
  return Number(props.globalParams[key] ?? 0);
}
</script>

<template>
  <aside class="props">
    <div class="props-tabs">
      <button
        v-for="t in TABS"
        :key="t.key"
        type="button"
        class="props-tab"
        :class="{ 'is-active': rightTab === t.key }"
        @click="emit('update:rightTab', t.key)"
      >
        {{ t.label }}
      </button>
      <button
        type="button"
        class="wb-icon-btn props-star"
        :class="{ 'is-on': isFavorite }"
        :title="isFavorite ? '取消收藏当前预设' : '收藏当前预设'"
        @click="emit('toggle-favorite')"
      >
        ★
      </button>
    </div>

    <div class="props-scroll wb-scroll">
      <template v-if="rightTab === 'props'">
        <LightParamsPanel
          :light="light"
          :baseline="baseline"
          @patch="emit('patch-light', $event)"
        />

        <ParamsGroup title="边缘与空气感" reset-label="重置" @reset="emit('reset-compositing')">
          <ParamSlider
            label="边缘光包裹"
            :model-value="compositingParams.light_wrap"
            :min="0"
            :max="1"
            :step="0.01"
            @update:model-value="emit('patch-compositing', { light_wrap: $event })"
          />
          <ParamSlider
            label="边缘光宽度"
            :model-value="compositingParams.rim_width"
            :min="1"
            :max="20"
            :step="0.5"
            :precision="1"
            @update:model-value="emit('patch-compositing', { rim_width: $event })"
          />
          <ParamSlider
            label="柔光泛光"
            :model-value="compositingParams.bloom_int"
            :min="0"
            :max="2"
            :step="0.01"
            @update:model-value="emit('patch-compositing', { bloom_int: $event })"
          />
          <ParamColor
            v-if="expandAir"
            label="泛光染色"
            :model-value="compositingParams.glow_color || '#ffffff'"
            @update:model-value="emit('patch-compositing', { glow_color: $event })"
          />

          <template v-if="expandAir">
            <ParamSlider
              label="泛光阈值"
              :model-value="compositingParams.bloom_thresh"
              :min="0.3"
              :max="0.95"
              :step="0.01"
              @update:model-value="emit('patch-compositing', { bloom_thresh: $event })"
            />
            <ParamSlider
              label="总强度"
              :model-value="globalNum('master')"
              :min="0"
              :max="2"
              :step="0.01"
              @update:model-value="emit('patch-global', { master: $event })"
            />
            <ParamSlider
              label="环境染色"
              :model-value="globalNum('ambient_blend')"
              :min="0"
              :max="1"
              :step="0.01"
              @update:model-value="emit('patch-global', { ambient_blend: $event })"
            />
            <ParamSlider
              label="暗位抬升"
              :model-value="globalNum('lift')"
              :min="0"
              :max="0.6"
              :step="0.01"
              @update:model-value="emit('patch-global', { lift: $event })"
            />
            <ParamSlider
              label="肤色保底"
              :model-value="globalNum('albedo_protect')"
              :min="0"
              :max="1"
              :step="0.01"
              hint="越高越去色：光只能改亮度、改不了颜色"
              @update:model-value="emit('patch-global', { albedo_protect: $event })"
            />
          </template>

          <button type="button" class="wb-more" @click="emit('update:expandAir', !expandAir)">
            {{ expandAir ? "收起参数" : "更多参数" }}
            <span style="font-size: 9px">{{ expandAir ? "‹" : "›" }}</span>
          </button>
        </ParamsGroup>

        <ParamsGroup
          title="体积光材质"
          note="Normal & Depth"
          reset-label="重置"
          @reset="emit('reset-material')"
        >
          <MapPreviewGrid
            :character-src="characterSrc"
            :normal-src="normalSrc"
            :depth-src="depthSrc"
            @update:normal-src="emit('update:normalSrc', $event)"
            @update:depth-src="emit('update:depthSrc', $event)"
          />

          <ParamSlider
            label="法线凹凸强度"
            :model-value="materialParams.normal_strength"
            :min="0"
            :max="2"
            :step="0.05"
            @update:model-value="emit('patch-material', { normal_strength: $event })"
          />
          <ParamSlider
            label="交界阴影强度"
            :model-value="materialParams.cel_thresh"
            :min="0.2"
            :max="0.8"
            :step="0.01"
            hint="二次元明暗交界线的切入位置"
            @update:model-value="emit('patch-material', { cel_thresh: $event })"
          />
          <ParamSlider
            label="深度阴影"
            :model-value="materialParams.self_shadow_int"
            :min="0"
            :max="1"
            :step="0.02"
            hint="用深度图做的接触投影，如刘海压在额头"
            @update:model-value="emit('patch-material', { self_shadow_int: $event })"
          />

          <template v-if="expandMaterial">
            <ParamSlider
              label="阴影羽化"
              :model-value="materialParams.cel_softness"
              :min="0.01"
              :max="0.3"
              :step="0.01"
              @update:model-value="emit('patch-material', { cel_softness: $event })"
            />
            <ParamSlider
              label="发丝高光"
              :model-value="materialParams.spec_int"
              :min="0"
              :max="1.5"
              :step="0.02"
              @update:model-value="emit('patch-material', { spec_int: $event })"
            />
            <ParamSlider
              label="高光聚焦"
              :model-value="materialParams.spec_gloss"
              :min="8"
              :max="64"
              :step="1"
              :precision="0"
              @update:model-value="emit('patch-material', { spec_gloss: $event })"
            />
            <ParamToggle
              label="使用法线贴图"
              :model-value="materialParams.use_normal"
              @update:model-value="emit('patch-material', { use_normal: $event })"
            />
            <ParamToggle
              label="使用深度贴图"
              :model-value="materialParams.use_depth"
              @update:model-value="emit('patch-material', { use_depth: $event })"
            />
            <button type="button" class="wb-more" @click="emit('reset-global')">
              恢复全局强度默认值
            </button>
          </template>

          <button
            type="button"
            class="wb-more"
            @click="emit('update:expandMaterial', !expandMaterial)"
          >
            {{ expandMaterial ? "收起参数" : "高级参数" }}
            <span style="font-size: 9px">{{ expandMaterial ? "‹" : "›" }}</span>
          </button>
        </ParamsGroup>
      </template>

      <HistoryPanel
        v-else
        :can-undo="canUndo"
        :can-redo="canRedo"
        :is-dirty="isDirty"
        :preset-name="presetName"
        @undo="emit('undo')"
        @redo="emit('redo')"
        @revert-preset="emit('revert-preset')"
      />
    </div>
  </aside>
</template>

<style scoped>
.props {
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--wb-surface);
  border-left: 1px solid var(--wb-border);
}

.props-tabs {
  display: flex;
  align-items: center;
  gap: 14px;
  height: 32px;
  padding: 0 10px;
  border-bottom: 1px solid var(--wb-border);
  flex-shrink: 0;
}

.props-tab {
  position: relative;
  height: 32px;
  font-size: 11.5px;
  color: var(--wb-muted);
  transition: color 0.12s ease;
}

.props-tab.is-active {
  color: var(--wb-text);
  font-weight: 600;
}

.props-tab.is-active::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 2px;
  border-radius: 2px;
  background: var(--wb-accent);
}

.props-star {
  margin-left: auto;
  font-size: 11px;
  color: var(--wb-border-strong);
}

.props-star.is-on {
  color: #f0a92f;
}

.props-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
</style>

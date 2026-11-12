<script setup lang="ts">
/**
 * 光影工作台外壳（Figma 稿版）。
 *
 * 职责只有三件：把 store 的文档态接到各组件、把组件的 intent 翻译成 store 调用、
 * 维护弹窗生命周期（初始化 / 快捷键 / 保存）。
 * 布局与控件状态在子组件里，Shell 不持有任何「散字段」。
 */
import { computed, onUnmounted, ref, watch } from "vue";
import type {
  LightSource,
  LightingCompositingParams,
  LightingGlobalParams,
  LightingMaterialParams,
  LightingParams,
} from "@/api/services/scene";
import { useSceneLighting } from "../useSceneLighting";
import { DEFAULT_COMPOSITING, DEFAULT_GLOBAL, DEFAULT_MATERIAL } from "../types";
import { MAX_LIGHTS } from "@/render/lighting/lights";
import LightingJsonModal from "../LightingJsonModal.vue";
import "./workbench.css";
import WorkbenchTopBar from "./WorkbenchTopBar.vue";
import WorkbenchStatusBar from "./WorkbenchStatusBar.vue";
import PresetList from "./PresetList.vue";
import LightList from "./LightList.vue";
import CanvasStage from "./CanvasStage.vue";
import PropertyPanel from "./PropertyPanel.vue";
import { useWorkbenchUi } from "./useWorkbenchUi";

const props = defineProps<{
  isOpen: boolean;
  sceneId?: string;
  sceneName?: string;
  backgroundSrc?: string | null;
  characterSrc?: string | null;
  initialLighting?: LightingParams | null;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "save", lighting: LightingParams): void;
}>();

const {
  lights,
  globalParams,
  compositingParams,
  materialParams,
  cameraParams,
  selectedLightIndex,
  selectedLight,
  curPresetId,
  currentPreset,
  presets,
  compareOriginal,
  isDirty,
  applyPreset,
  revertCurrentPreset,
  initFromLighting,
  setShot,
  addLight,
  removeLight,
  moveLight,
  toggleKeyLight,
  toLightingParams,
  exportJson,
  importJson,
  undo,
  redo,
  canUndo,
  canRedo,
  favoritePresetIds,
  toggleFavoritePreset,
  createCustomPreset,
} = useSceneLighting();

const {
  rightTab,
  canvasTool,
  showGizmo,
  showGrid,
  zoom,
  expandAir,
  expandMaterial,
  toastMessage,
  toast,
} = useWorkbenchUi();

/** 用户上传的法线/深度图；null = 自动生成 */
const customNormal = ref<string | null>(null);
const customDepth = ref<string | null>(null);
/** 框选命中的灯 id（只做高亮，选中仍以 selectedLightIndex 为准） */
const highlightIds = ref<string[]>([]);
const showJsonModal = ref(false);
const jsonInitialText = ref("");
const saveOpen = ref(false);
const saveName = ref("");

const currentFavorite = computed(() => favoritePresetIds.value.includes(curPresetId.value));

/** 预设里同序号的灯，作为双击复位的基准 */
const baselineLight = computed<LightSource | null>(() => {
  const p = currentPreset.value;
  if (!p) return null;
  return p.lights[selectedLightIndex.value] ?? null;
});

const presetDisplayName = computed(() => {
  const n = currentPreset.value?.name ?? "自定义";
  return n.replace(/^\s*\d+\s*[·.、]\s*/, "");
});

// —— 文档态写入 ——

function patchLight(patch: Partial<LightSource>) {
  const l = selectedLight.value;
  if (!l) return;
  Object.assign(l, patch);
  isDirty.value = true;
}

function patchGlobal(patch: Partial<LightingGlobalParams>) {
  Object.assign(globalParams.value, patch);
  isDirty.value = true;
}

function patchCompositing(patch: Partial<LightingCompositingParams>) {
  Object.assign(compositingParams.value, patch);
  isDirty.value = true;
}

function patchMaterial(patch: Partial<LightingMaterialParams>) {
  Object.assign(materialParams.value, patch);
  isDirty.value = true;
}

function resetCompositing() {
  compositingParams.value = {
    ...(currentPreset.value?.compositing ?? DEFAULT_COMPOSITING),
  };
  toast("已重置边缘与空气感");
}

function resetMaterial() {
  materialParams.value = { ...(currentPreset.value?.material ?? DEFAULT_MATERIAL) };
  toast("已重置体积光材质");
}

function resetGlobal() {
  globalParams.value = { ...(currentPreset.value?.global ?? DEFAULT_GLOBAL) };
  toast("已重置全局强度");
}

// —— 灯光增删 ——

function onLightPos(payload: { id: string; x: number; y: number }) {
  const l = lights.value.find((it) => it.id === payload.id);
  if (!l) return;
  l.x = payload.x;
  l.y = payload.y;
  isDirty.value = true;
}

function onPlaceLight(pos: { x: number; y: number }) {
  const ok = addLight("point", pos);
  toast(
    ok ? `已放一盏点光（${pos.x.toFixed(2)}, ${pos.y.toFixed(2)}）` : `最多 ${MAX_LIGHTS} 盏灯`,
  );
}

/** 框选：命中的灯高亮，选中离矩形中心最近那盏 */
function onMarquee(box: { x0: number; y0: number; x1: number; y1: number }) {
  const cx = (box.x0 + box.x1) / 2;
  const cy = (box.y0 + box.y1) / 2;
  const hit: number[] = [];
  lights.value.forEach((l, i) => {
    const placeable = l.type === "point" || l.type === "spot";
    if (!placeable || l.enabled === false) return;
    if (l.x >= box.x0 && l.x <= box.x1 && l.y >= box.y0 && l.y <= box.y1) hit.push(i);
  });
  highlightIds.value = hit.map((i) => lights.value[i].id);
  if (!hit.length) {
    toast("框选范围内没有光源");
    return;
  }
  let best = hit[0];
  let bestD = Number.POSITIVE_INFINITY;
  hit.forEach((i) => {
    const l = lights.value[i];
    const d = (l.x - cx) ** 2 + (l.y - cy) ** 2;
    if (d < bestD) {
      bestD = d;
      best = i;
    }
  });
  selectedLightIndex.value = best;
  toast(`框选到 ${hit.length} 盏灯`);
}

function onSwitchTab(tab: "lighting" | "assets") {
  if (tab === "assets") toast("素材管理尚未接入");
}

function onAddLight(type: LightSource["type"]) {
  if (!addLight(type)) toast(`最多 ${MAX_LIGHTS} 盏灯`);
}

function onDuplicateLight(index: number) {
  if (lights.value.length >= MAX_LIGHTS) {
    toast(`最多 ${MAX_LIGHTS} 盏灯`);
    return;
  }
  const src = lights.value[index];
  if (!src) return;
  const copy = JSON.parse(JSON.stringify(src)) as LightSource;
  copy.id = `light-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
  copy.name = `${src.name || "光源"} 副本`;
  copy.is_key = false;
  lights.value.splice(index + 1, 0, copy);
  selectedLightIndex.value = index + 1;
  isDirty.value = true;
}

function onToggleEnabled(index: number) {
  const l = lights.value[index];
  if (!l) return;
  l.enabled = l.enabled === false;
  isDirty.value = true;
}

function onRemoveLight(index: number) {
  if (lights.value.length <= 1) {
    toast("至少保留一盏灯");
    return;
  }
  removeLight(index);
}

function onMoveLight(index: number, dir: "up" | "down") {
  moveLight(index, dir);
}

function onSelectPreset(id: string) {
  applyPreset(id);
}

// —— 顶栏动作 ——

function openExport() {
  jsonInitialText.value = exportJson();
  showJsonModal.value = true;
}

function openImport() {
  jsonInitialText.value = "";
  showJsonModal.value = true;
}

const MAX_IMPORT_CHARS = 2_000_000;

function applyJson(raw: string): boolean {
  if (!raw || !raw.trim()) {
    toast("导入失败：内容为空");
    return false;
  }
  if (raw.length > MAX_IMPORT_CHARS) {
    toast(`导入失败：JSON 超过 ${Math.round(MAX_IMPORT_CHARS / 1000)}k 字符`);
    return false;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    toast(`导入失败：不是合法 JSON（${(err as Error).message.slice(0, 40)}）`);
    return false;
  }
  const result = importJson(parsed);
  toast(result.ok ? `✓ ${result.message}` : `导入失败：${result.message}`);
  return result.ok;
}

/** 导入弹窗只在成功后关：失败时留着文本，用户能照着原因改完再试 */
function onApplyJson(raw: string) {
  if (applyJson(raw)) showJsonModal.value = false;
}

function openSaveDialog() {
  saveName.value = `${presetDisplayName.value} 副本`;
  saveOpen.value = true;
}

function confirmSavePreset() {
  const name = saveName.value.trim();
  if (!name) return;
  createCustomPreset(name);
  saveOpen.value = false;
  toast(`已保存为「${name}」`);
}

function onRevertPreset() {
  if (revertCurrentPreset()) toast("已恢复到该预设的出厂参数");
}

function handleSave() {
  if (lights.value.length === 0) applyPreset("neutral");
  emit("save", toLightingParams(props.initialLighting || undefined));
  emit("close");
}

// —— 快捷键 ——

function onKeydown(e: KeyboardEvent) {
  if (!props.isOpen) return;
  const target = e.target as HTMLElement | null;
  if (target?.isContentEditable) return;
  if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) {
    // 只接管滑块上的 Ctrl+Z，文本框的撤销让给原生
    if (target.getAttribute("type") !== "range") return;
  }
  const mod = e.ctrlKey || e.metaKey;
  if (!mod) return;
  const k = e.key.toLowerCase();
  if (k === "z") {
    e.preventDefault();
    if (e.shiftKey) {
      if (redo()) toast("↷ 重做");
    } else if (undo()) {
      toast("↶ 撤销");
    }
  } else if (k === "y") {
    e.preventDefault();
    if (redo()) toast("↷ 重做");
  }
}

watch(
  () => props.isOpen,
  (open) => {
    if (open) {
      initFromLighting(props.initialLighting);
      window.addEventListener("keydown", onKeydown);
    } else {
      window.removeEventListener("keydown", onKeydown);
      customNormal.value = null;
      customDepth.value = null;
      highlightIds.value = [];
      saveOpen.value = false;
    }
  },
  { immediate: true },
);

onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div v-if="isOpen" class="overlay" @click.self="emit('close')">
      <div class="wb-root wb-window">
        <WorkbenchTopBar
          :scene-name="sceneName"
          :dirty="isDirty"
          :can-undo="canUndo"
          :can-redo="canRedo"
          @undo="undo"
          @redo="redo"
          @import="openImport"
          @export="openExport"
          @close="emit('close')"
          @switch-tab="onSwitchTab"
        />

        <div class="wb-body">
          <aside class="wb-left">
            <PresetList
              class="wb-left-presets"
              :presets="presets"
              :cur-preset-id="curPresetId"
              :favorite-ids="favoritePresetIds"
              @select="onSelectPreset"
              @toggle-favorite="toggleFavoritePreset"
              @save-as="openSaveDialog"
            />
            <LightList
              class="wb-left-lights"
              :lights="lights"
              :selected-index="selectedLightIndex"
              @select="selectedLightIndex = $event"
              @add="onAddLight"
              @remove="onRemoveLight"
              @duplicate="onDuplicateLight"
              @toggle-enabled="onToggleEnabled"
              @toggle-key="toggleKeyLight"
              @move="onMoveLight"
            />
          </aside>

          <CanvasStage
            class="wb-center"
            :background-src="backgroundSrc"
            :character-src="characterSrc"
            :lights="lights"
            :global-params="globalParams"
            :compositing-params="compositingParams"
            :material-params="materialParams"
            :camera-params="cameraParams"
            :selected-light-index="selectedLightIndex"
            :compare-original="compareOriginal"
            :scene-name="sceneName"
            :preset-name="presetDisplayName"
            :zoom="zoom"
            :show-gizmo="showGizmo"
            :show-grid="showGrid"
            :tool="canvasTool"
            :normal-src="customNormal"
            :depth-src="customDepth"
            :highlight-ids="highlightIds"
            @update:selected-light-index="selectedLightIndex = $event"
            @update:zoom="zoom = $event"
            @update:tool="canvasTool = $event"
            @update:show-gizmo="showGizmo = $event"
            @update:show-grid="showGrid = $event"
            @update:light-pos="onLightPos"
            @set-shot="setShot"
            @place-light="onPlaceLight"
            @marquee="onMarquee"
          />

          <PropertyPanel
            class="wb-right"
            :right-tab="rightTab"
            :light="selectedLight"
            :baseline-light="baselineLight"
            :global-params="globalParams"
            :compositing-params="compositingParams"
            :material-params="materialParams"
            :character-src="characterSrc"
            :normal-src="customNormal"
            :depth-src="customDepth"
            :expand-air="expandAir"
            :expand-material="expandMaterial"
            :preset-name="presetDisplayName"
            :is-favorite="currentFavorite"
            :can-undo="canUndo"
            :can-redo="canRedo"
            :is-dirty="isDirty"
            @update:right-tab="rightTab = $event"
            @update:expand-air="expandAir = $event"
            @update:expand-material="expandMaterial = $event"
            @patch-light="patchLight"
            @patch-global="patchGlobal"
            @patch-compositing="patchCompositing"
            @patch-material="patchMaterial"
            @reset-compositing="resetCompositing"
            @reset-material="resetMaterial"
            @reset-global="resetGlobal"
            @update:normal-src="customNormal = $event"
            @update:depth-src="customDepth = $event"
            @toggle-favorite="toggleFavoritePreset(curPresetId)"
            @undo="undo"
            @redo="redo"
            @revert-preset="onRevertPreset"
          />
        </div>

        <WorkbenchStatusBar />

        <!-- 底部动作区（保存 / 取消） -->
        <div class="wb-footer">
          <span class="wb-footer-hint">
            {{ lights.length }} 盏灯 · {{ rightTab === "props" ? "属性" : "历史记录" }}
          </span>
          <button type="button" class="wb-btn" @click="emit('close')">取消</button>
          <button type="button" class="wb-btn wb-btn-primary" @click="handleSave">
            保存到场景
          </button>
        </div>

        <!-- 保存为预设 -->
        <div v-if="saveOpen" class="wb-dialog-mask" @click.self="saveOpen = false">
          <div class="wb-dialog">
            <p class="wb-dialog-title">保存为预设</p>
            <input
              v-model="saveName"
              class="wb-input"
              style="width: 100%"
              placeholder="预设名称"
              @keydown.enter="confirmSavePreset"
            />
            <div class="wb-dialog-actions">
              <button type="button" class="wb-btn" @click="saveOpen = false">取消</button>
              <button type="button" class="wb-btn wb-btn-primary" @click="confirmSavePreset">
                保存
              </button>
            </div>
          </div>
        </div>

        <transition name="wb-toast">
          <div v-if="toastMessage" class="wb-toast">{{ toastMessage }}</div>
        </transition>
      </div>

      <LightingJsonModal
        :is-open="showJsonModal"
        :initial-json="jsonInitialText"
        :all-presets-json="JSON.stringify({ version: 4, presets }, null, 2)"
        @close="showJsonModal = false"
        @apply="onApplyJson"
      />
    </div>
  </Teleport>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 12px;
  background: rgb(9 11 16 / 72%);
  backdrop-filter: blur(6px);
}

.wb-window {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  max-width: 1360px;
  height: 100%;
  max-height: 92vh;
  overflow: hidden;
  border-radius: 14px;
  border: 1px solid rgb(255 255 255 / 55%);
  box-shadow: 0 24px 80px rgb(0 0 0 / 45%);
}

.wb-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.wb-left {
  display: flex;
  flex-direction: column;
  width: 208px;
  flex-shrink: 0;
  min-height: 0;
  border-right: 1px solid var(--wb-border);
  background: var(--wb-surface);
}

.wb-left-presets {
  flex: 1;
  min-height: 0;
}

.wb-left-lights {
  max-height: 42%;
  flex-shrink: 0;
}

.wb-center {
  flex: 1;
  min-width: 0;
}

.wb-right {
  width: 268px;
  flex-shrink: 0;
}

.wb-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  height: 40px;
  padding: 0 12px;
  background: var(--wb-surface);
  border-top: 1px solid var(--wb-border);
  flex-shrink: 0;
}

.wb-footer-hint {
  margin-right: auto;
  font-size: 10.5px;
  color: var(--wb-muted);
}

.wb-dialog-mask {
  position: absolute;
  inset: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgb(23 23 28 / 32%);
  backdrop-filter: blur(2px);
}

.wb-dialog {
  width: 280px;
  padding: 14px;
  border-radius: var(--wb-r-lg);
  background: var(--wb-surface);
  box-shadow: 0 18px 50px rgb(23 23 28 / 25%);
}

.wb-dialog-title {
  font-size: 12.5px;
  font-weight: 600;
  margin-bottom: 9px;
}

.wb-dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 12px;
}

.wb-toast {
  position: absolute;
  left: 50%;
  bottom: 54px;
  transform: translateX(-50%);
  padding: 5px 12px;
  border-radius: 999px;
  background: rgb(23 23 28 / 90%);
  color: #fff;
  font-size: 11px;
  box-shadow: 0 8px 24px rgb(0 0 0 / 28%);
}

.wb-toast-enter-active,
.wb-toast-leave-active {
  transition:
    opacity 0.16s ease,
    transform 0.16s ease;
}

.wb-toast-enter-from,
.wb-toast-leave-to {
  opacity: 0;
  transform: translate(-50%, 6px);
}
</style>

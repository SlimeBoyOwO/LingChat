<script setup lang="ts">
import { ref, watch, computed, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import type { LightingParams, LightingShot } from "@/api/services/scene";
import { useSceneLighting } from "./lighting/useSceneLighting";
import LightingCanvasStage from "./lighting/LightingCanvasStage.vue";
import LightingPresetGrid from "./lighting/LightingPresetGrid.vue";
import LightingLightsManager from "./lighting/LightingLightsManager.vue";
import LightingMaterialInspector from "./lighting/LightingMaterialInspector.vue";
import LightingAtmosphereInspector from "./lighting/LightingAtmosphereInspector.vue";
import LightingJsonModal from "./lighting/LightingJsonModal.vue";

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
  compareOriginal,
  presets,
  applyPreset,
  overrideCurrentPreset,
  revertCurrentPreset,
  initFromLighting,
  setShot,
  addLight,
  removeLight,
  toggleKeyLight,
  moveLight,
  toLightingParams,
  exportJson,
  importJson,
  undo,
  redo,
  canUndo,
  canRedo,
  favoritePresetIds,
  toggleFavoritePreset,
} = useSceneLighting();

// Active inspector tab
type TabKey = "presets" | "lights" | "material" | "compositing";
const { t } = useI18n();
const activeTab = ref<TabKey>("presets");

// JSON modal visibility
const showJsonModal = ref(false);
const jsonInitialText = ref("");

// Custom background upload
const customBackgroundSrc = ref<string | null>(null);
const bgFileInputRef = ref<HTMLInputElement | null>(null);

function onCustomBgSelected(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = (evt) => {
    customBackgroundSrc.value = String(evt.target?.result || "");
    showToast(t("settings.sceneEdit.lightingWorkbench.toast.customBgLoaded", { name: file.name }));
  };
  reader.readAsDataURL(file);
}

// Notification toast
const toastMessage = ref("");
let toastTimer: ReturnType<typeof setTimeout> | null = null;
function showToast(msg: string) {
  toastMessage.value = msg;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastMessage.value = "";
  }, 2200);
}

function handleKeydown(e: KeyboardEvent) {
  if (!props.isOpen) return;
  const target = e.target as HTMLElement | null;
  if (
    target &&
    (target.tagName === "INPUT" || target.tagName === "TEXTAREA") &&
    target.getAttribute("type") !== "range"
  ) {
    return;
  }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
    e.preventDefault();
    if (e.shiftKey) {
      if (redo()) showToast("↷ " + t("settings.sceneEdit.lightingWorkbench.redo"));
    } else {
      if (undo()) showToast("↶ " + t("settings.sceneEdit.lightingWorkbench.undo"));
    }
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") {
    e.preventDefault();
    if (redo()) showToast("↷ " + t("settings.sceneEdit.lightingWorkbench.redo"));
  }
}

// Watch isOpen to load initial data & attach listeners
watch(
  () => props.isOpen,
  (open) => {
    if (open) {
      initFromLighting(props.initialLighting);
      window.addEventListener("keydown", handleKeydown);
    } else {
      window.removeEventListener("keydown", handleKeydown);
    }
  },
  { immediate: true },
);

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeydown);
});

function handleLightPos(payload: { index: number; x: number; y: number }) {
  const l = lights.value[payload.index];
  if (l) {
    l.x = payload.x;
    l.y = payload.y;
  }
}

function handleLightDirection(payload: { index: number; azimuth: number; elevation: number }) {
  const l = lights.value[payload.index];
  if (l) {
    l.azimuth = payload.azimuth;
    l.elevation = payload.elevation;
  }
}

function onOverridePreset() {
  if (overrideCurrentPreset()) {
    showToast(t("settings.sceneEdit.lightingWorkbench.toast.copied"));
  }
}

function onRevertPreset() {
  if (revertCurrentPreset()) {
    showToast("✓ " + t("settings.sceneEdit.lightingWorkbench.presets.revertPreset"));
  }
}

function openExportJson() {
  jsonInitialText.value = exportJson();
  showJsonModal.value = true;
}

function openImportJson() {
  jsonInitialText.value = "";
  showJsonModal.value = true;
}

function handleApplyJson(rawStr: string) {
  try {
    const parsed = JSON.parse(rawStr);
    if (importJson(parsed)) {
      showToast("✓ 成功导入并应用光影参数");
    }
  } catch (err: unknown) {
    showToast(`导入失败: ${(err as Error).message}`);
  }
}

const allPresetsJson = computed(() => {
  return JSON.stringify({ version: "4.0", presets: presets.value }, null, 2);
});

function handleSave() {
  const finalParams = toLightingParams(props.initialLighting || undefined);
  emit("save", finalParams);
  emit("close");
}
</script>

<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-2 backdrop-blur-md sm:p-4"
    @click.self="emit('close')"
  >
    <div
      class="flex h-full max-h-[92vh] w-full max-w-[1380px] flex-col overflow-hidden rounded-2xl border border-[#232b3d] bg-[#0e1117] shadow-2xl"
    >
      <!-- Topbar Header -->
      <header
        class="flex h-13 shrink-0 items-center justify-between border-b border-[#1b212f] bg-[#121620] px-4"
      >
        <div class="flex items-center gap-3">
          <div class="flex items-center gap-2">
            <span
              class="rounded bg-gradient-to-r from-purple-600 to-indigo-600 px-2 py-0.5 text-[10px] font-bold tracking-wider text-white uppercase"
            >
              PRO-STUDIO
            </span>
            <h2 class="text-sm font-bold text-slate-100">
              {{ $t("settings.sceneEdit.lightingWorkbench.title") }}
            </h2>
          </div>
          <span
            v-if="sceneName"
            class="hidden rounded border border-purple-500/30 bg-purple-500/10 px-2 py-0.5 text-[11px] text-purple-300 md:inline-block"
          >
            {{ $t("settings.sceneEdit.lightingWorkbench.sceneBadge", { name: sceneName }) }}
          </span>
        </div>

        <div class="flex items-center gap-2">
          <!-- Undo / Redo controls -->
          <div class="mr-0.5 flex items-center gap-1 border-r border-[#2b3347] pr-2">
            <button
              type="button"
              @click="undo"
              :disabled="!canUndo"
              class="flex items-center gap-1 rounded border border-[#2b3347] bg-[#171c26] px-2 py-1 text-xs font-medium text-slate-300 transition-all hover:bg-[#202736] hover:text-white disabled:pointer-events-none disabled:opacity-30"
              :title="$t('settings.sceneEdit.lightingWorkbench.undoTitle')"
            >
              ↶ {{ $t("settings.sceneEdit.lightingWorkbench.undo") }}
            </button>
            <button
              type="button"
              @click="redo"
              :disabled="!canRedo"
              class="flex items-center gap-1 rounded border border-[#2b3347] bg-[#171c26] px-2 py-1 text-xs font-medium text-slate-300 transition-all hover:bg-[#202736] hover:text-white disabled:pointer-events-none disabled:opacity-30"
              :title="$t('settings.sceneEdit.lightingWorkbench.redoTitle')"
            >
              ↷ {{ $t("settings.sceneEdit.lightingWorkbench.redo") }}
            </button>
          </div>

          <button
            type="button"
            @click="openImportJson"
            class="rounded border border-[#2b3347] bg-[#171c26] px-2.5 py-1 text-xs font-medium text-slate-300 hover:bg-[#202736] hover:text-white"
          >
            {{ $t("settings.sceneEdit.lightingWorkbench.btnImportJson") }}
          </button>
          <button
            type="button"
            @click="openExportJson"
            class="rounded border border-[#2b3347] bg-[#171c26] px-2.5 py-1 text-xs font-medium text-slate-300 hover:bg-[#202736] hover:text-white"
          >
            {{ $t("settings.sceneEdit.lightingWorkbench.btnExportJson") }}
          </button>
          <button
            type="button"
            @click="emit('close')"
            class="rounded px-3 py-1 text-xs text-slate-400 hover:text-slate-200"
          >
            {{ $t("settings.sceneEdit.lightingWorkbench.btnCancel") }}
          </button>
          <button
            type="button"
            @click="handleSave"
            class="rounded bg-purple-600 px-4 py-1 text-xs font-bold text-white shadow-sm hover:bg-purple-500 active:scale-95"
          >
            {{ $t("settings.sceneEdit.lightingWorkbench.btnSave") }}
          </button>
        </div>
      </header>

      <!-- Main Two-Column Workbench Body -->
      <div class="flex flex-1 flex-col overflow-hidden lg:flex-row">
        <!-- Left: Stage & WebGL Canvas -->
        <div
          class="flex flex-1 flex-col overflow-hidden border-b border-[#283042] lg:border-r lg:border-b-0"
        >
          <LightingCanvasStage
            :background-src="customBackgroundSrc || backgroundSrc"
            :character-src="characterSrc"
            :current-preset-name="currentPreset?.name"
            :lights="lights"
            :global-params="globalParams"
            :compositing-params="compositingParams"
            :material-params="materialParams"
            :camera-params="cameraParams"
            :selected-light-index="selectedLightIndex"
            :compare-original="compareOriginal"
            @update:background-src="customBackgroundSrc = $event"
            @update:selected-light-index="selectedLightIndex = $event"
            @update:light-pos="handleLightPos"
            @update:light-direction="handleLightDirection"
            @set-shot="setShot"
          />
        </div>

        <!-- Right: Inspector Panels with Tab Bar -->
        <div class="flex w-full flex-col overflow-hidden bg-[#0a0d13] lg:w-[460px]">
          <!-- Tab Navigation -->
          <div class="grid grid-cols-4 border-b border-[#1b212f] bg-[#10141d] text-center text-xs">
            <button
              type="button"
              @click="activeTab = 'presets'"
              :class="[
                'py-2.5 font-semibold transition-colors',
                activeTab === 'presets'
                  ? 'border-b-2 border-purple-500 text-purple-300'
                  : 'text-slate-400 hover:text-slate-200',
              ]"
            >
              预设矩阵
            </button>
            <button
              type="button"
              @click="activeTab = 'lights'"
              :class="[
                'py-2.5 font-semibold transition-colors',
                activeTab === 'lights'
                  ? 'border-b-2 border-purple-500 text-purple-300'
                  : 'text-slate-400 hover:text-slate-200',
              ]"
            >
              灯光 ({{ lights.length }})
            </button>
            <button
              type="button"
              @click="activeTab = 'material'"
              :class="[
                'py-2.5 font-semibold transition-colors',
                activeTab === 'material'
                  ? 'border-b-2 border-purple-500 text-purple-300'
                  : 'text-slate-400 hover:text-slate-200',
              ]"
            >
              2.5D 材质
            </button>
            <button
              type="button"
              @click="activeTab = 'compositing'"
              :class="[
                'py-2.5 font-semibold transition-colors',
                activeTab === 'compositing'
                  ? 'border-b-2 border-purple-500 text-purple-300'
                  : 'text-slate-400 hover:text-slate-200',
              ]"
            >
              2D 合成
            </button>
          </div>

          <!-- Tab Content Scrollable Area -->
          <div
            class="flex-1 overflow-y-auto p-4"
            style="scrollbar-width: thin; scrollbar-color: #2b3347 transparent"
          >
            <!-- Presets Tab -->
            <div v-show="activeTab === 'presets'">
              <LightingPresetGrid
                :presets="presets"
                :cur-preset-id="curPresetId"
                :favorite-preset-ids="favoritePresetIds"
                @select-preset="applyPreset"
                @override-cur="onOverridePreset"
                @revert-cur="onRevertPreset"
                @toggle-favorite="toggleFavoritePreset"
              />
            </div>

            <!-- Lights Tab -->
            <div v-show="activeTab === 'lights'">
              <LightingLightsManager
                :lights="lights"
                :selected-light-index="selectedLightIndex"
                :selected-light="selectedLight"
                @update:selected-light-index="selectedLightIndex = $event"
                @add-light="addLight"
                @remove-light="removeLight"
                @toggle-key-light="toggleKeyLight"
                @move-light="moveLight"
              />
            </div>

            <!-- 2.5D Material Tab -->
            <div v-show="activeTab === 'material'">
              <LightingMaterialInspector
                :material-params="materialParams"
                @reset="showToast('✓ 材质参数已重置')"
              />
            </div>

            <!-- 2D Compositing & Tone Tab -->
            <div v-show="activeTab === 'compositing'">
              <LightingAtmosphereInspector
                :compositing-params="compositingParams"
                :global-params="globalParams"
                @reset-compositing="showToast('✓ 合成参数已重置')"
                @reset-global="showToast('✓ 全局参数已重置')"
              />
            </div>
          </div>
        </div>
      </div>

      <!-- Toast Feedback -->
      <transition
        enter-active-class="transition duration-200 ease-out"
        enter-from-class="translate-y-2 opacity-0"
        enter-to-class="translate-y-0 opacity-100"
        leave-active-class="transition duration-150 ease-in"
        leave-from-class="opacity-100"
        leave-to-class="opacity-0"
      >
        <div
          v-if="toastMessage"
          class="absolute bottom-6 left-1/2 -translate-x-1/2 rounded-full border border-purple-500/40 bg-purple-950/90 px-4 py-1.5 text-xs font-semibold text-purple-200 shadow-xl backdrop-blur-md"
        >
          {{ toastMessage }}
        </div>
      </transition>
    </div>

    <!-- JSON Modal -->
    <LightingJsonModal
      :is-open="showJsonModal"
      :initial-json="jsonInitialText"
      :all-presets-json="allPresetsJson"
      @close="showJsonModal = false"
      @apply="handleApplyJson"
    />
  </div>
</template>

import { ref, computed, watch } from "vue";
import type {
  LightSource,
  LightType,
  LightingCameraParams,
  LightingCompositingParams,
  LightingGlobalParams,
  LightingMaterialParams,
  LightingParams,
  LightingShot,
} from "@/api/services/scene";
import {
  DEFAULT_CAMERA,
  DEFAULT_COMPOSITING,
  DEFAULT_GLOBAL,
  DEFAULT_MATERIAL,
  DEFAULT_NEW_LIGHT,
  SHOT_CAMERA_MAP,
  type LightingPreset,
} from "./types";
import { BUILTIN_PRESETS } from "./presetsData";

const LS_OVERRIDE_KEY = "lingchat.lighting.builtinOverrides";
const LS_FAV_KEY = "lingchat.lighting.favoritePresets";

interface LightingSnapshot {
  lights: LightSource[];
  global: LightingGlobalParams;
  compositing: LightingCompositingParams;
  material: LightingMaterialParams;
  camera: LightingCameraParams;
  selectedLightIndex: number;
  curPresetId: string;
}

export function useSceneLighting() {
  const lights = ref<LightSource[]>([]);
  const globalParams = ref<LightingGlobalParams>({ ...DEFAULT_GLOBAL });
  const compositingParams = ref<LightingCompositingParams>({
    ...DEFAULT_COMPOSITING,
  });
  const materialParams = ref<LightingMaterialParams>({ ...DEFAULT_MATERIAL });
  const cameraParams = ref<LightingCameraParams>({ ...DEFAULT_CAMERA });

  const selectedLightIndex = ref<number>(0);
  const curPresetId = ref<string>("neutral");
  const compareOriginal = ref<boolean>(false);
  const isDirty = ref<boolean>(false);

  // Deep clone baseline presets
  const presets = ref<LightingPreset[]>(JSON.parse(JSON.stringify(BUILTIN_PRESETS)));

  // Favorite presets persistence
  const favoritePresetIds = ref<string[]>([]);
  function loadFavorites() {
    try {
      const raw = localStorage.getItem(LS_FAV_KEY);
      if (raw) favoritePresetIds.value = JSON.parse(raw);
    } catch {}
  }
  loadFavorites();

  function toggleFavoritePreset(id: string) {
    const idx = favoritePresetIds.value.indexOf(id);
    if (idx >= 0) {
      favoritePresetIds.value.splice(idx, 1);
    } else {
      favoritePresetIds.value.push(id);
    }
    try {
      localStorage.setItem(LS_FAV_KEY, JSON.stringify(favoritePresetIds.value));
    } catch {}
  }

  function isPresetFavorite(id: string): boolean {
    return favoritePresetIds.value.includes(id);
  }

  // History stack engine (Undo / Redo)
  const HISTORY_LIMIT = 30;
  const undoStack = ref<LightingSnapshot[]>([]);
  const redoStack = ref<LightingSnapshot[]>([]);
  const isRestoringHistory = ref(false);

  const canUndo = computed(() => undoStack.value.length > 0);
  const canRedo = computed(() => redoStack.value.length > 0);

  function captureSnapshot(): LightingSnapshot {
    return {
      lights: JSON.parse(JSON.stringify(lights.value)),
      global: { ...globalParams.value },
      compositing: { ...compositingParams.value },
      material: { ...materialParams.value },
      camera: { ...cameraParams.value },
      selectedLightIndex: selectedLightIndex.value,
      curPresetId: curPresetId.value,
    };
  }

  function commitHistory() {
    if (isRestoringHistory.value) return;
    const snap = captureSnapshot();
    undoStack.value.push(snap);
    if (undoStack.value.length > HISTORY_LIMIT) {
      undoStack.value.shift();
    }
    redoStack.value = [];
    isDirty.value = true;
  }

  function applySnapshot(snap: LightingSnapshot) {
    lights.value = JSON.parse(JSON.stringify(snap.lights));
    globalParams.value = { ...snap.global };
    compositingParams.value = { ...snap.compositing };
    materialParams.value = { ...snap.material };
    cameraParams.value = { ...snap.camera };
    selectedLightIndex.value = Math.max(
      0,
      Math.min(snap.selectedLightIndex, lights.value.length - 1),
    );
    curPresetId.value = snap.curPresetId;
  }

  function undo(): boolean {
    if (undoStack.value.length === 0) return false;
    isRestoringHistory.value = true;
    redoStack.value.push(captureSnapshot());
    const prev = undoStack.value.pop()!;
    applySnapshot(prev);
    isRestoringHistory.value = false;
    return true;
  }

  function redo(): boolean {
    if (redoStack.value.length === 0) return false;
    isRestoringHistory.value = true;
    undoStack.value.push(captureSnapshot());
    const next = redoStack.value.pop()!;
    applySnapshot(next);
    isRestoringHistory.value = false;
    return true;
  }

  let historyTimer: ReturnType<typeof setTimeout> | null = null;
  watch(
    [lights, globalParams, compositingParams, materialParams, cameraParams],
    () => {
      if (isRestoringHistory.value) return;
      if (historyTimer) clearTimeout(historyTimer);
      historyTimer = setTimeout(() => {
        commitHistory();
      }, 600);
    },
    { deep: true },
  );

  // Load localStorage overrides on init
  function loadOverrides() {
    try {
      const stored = localStorage.getItem(LS_OVERRIDE_KEY);
      if (!stored) return;
      const overrides = JSON.parse(stored);
      presets.value.forEach((p) => {
        if (overrides[p.id]) {
          Object.assign(p, overrides[p.id]);
          p._overridden = true;
        }
      });
    } catch {
      // Ignore storage parse errors
    }
  }
  loadOverrides();

  const selectedLight = computed<LightSource | null>(() => {
    if (selectedLightIndex.value >= 0 && selectedLightIndex.value < lights.value.length) {
      return lights.value[selectedLightIndex.value];
    }
    return null;
  });

  const currentPreset = computed(() => presets.value.find((p) => p.id === curPresetId.value));

  // Apply a preset
  function applyPreset(presetId: string) {
    const p = presets.value.find((item) => item.id === presetId) || presets.value[0];
    if (!p) return;

    curPresetId.value = p.id;
    cameraParams.value = { ...p.camera };
    globalParams.value = { ...p.global };
    compositingParams.value = { ...p.compositing };
    materialParams.value = { ...p.material };
    lights.value = JSON.parse(JSON.stringify(p.lights));

    selectedLightIndex.value = Math.min(
      selectedLightIndex.value,
      Math.max(0, lights.value.length - 1),
    );
    isDirty.value = true;
  }

  // Override current preset to localStorage
  function overrideCurrentPreset(): boolean {
    const cur = presets.value.find((p) => p.id === curPresetId.value);
    if (!cur) return false;

    const patch: Partial<LightingPreset> = {
      camera: { ...cameraParams.value },
      global: { ...globalParams.value },
      compositing: { ...compositingParams.value },
      material: { ...materialParams.value },
      lights: JSON.parse(JSON.stringify(lights.value)),
    };
    Object.assign(cur, patch);
    cur._overridden = true;

    try {
      const stored = localStorage.getItem(LS_OVERRIDE_KEY);
      const overrides = stored ? JSON.parse(stored) : {};
      overrides[cur.id] = patch;
      localStorage.setItem(LS_OVERRIDE_KEY, JSON.stringify(overrides));
      return true;
    } catch {
      return false;
    }
  }

  // Revert preset to factory baseline
  function revertCurrentPreset(): boolean {
    const cur = presets.value.find((p) => p.id === curPresetId.value);
    const orig = BUILTIN_PRESETS.find((p) => p.id === curPresetId.value);
    if (!cur || !orig) return false;

    Object.assign(cur, JSON.parse(JSON.stringify(orig)));
    delete cur._overridden;

    try {
      const stored = localStorage.getItem(LS_OVERRIDE_KEY);
      if (stored) {
        const overrides = JSON.parse(stored);
        delete overrides[cur.id];
        localStorage.setItem(LS_OVERRIDE_KEY, JSON.stringify(overrides));
      }
    } catch {}

    applyPreset(cur.id);
    return true;
  }

  // Initialize from existing SceneInfo.lighting or default
  function initFromLighting(initial: LightingParams | null | undefined) {
    if (initial?.lights && Array.isArray(initial.lights)) {
      lights.value = JSON.parse(JSON.stringify(initial.lights));
      if (initial.global) globalParams.value = { ...initial.global };
      if (initial.compositing) compositingParams.value = { ...initial.compositing };
      if (initial.material) materialParams.value = { ...initial.material };
      if (initial.camera) cameraParams.value = { ...initial.camera };
      selectedLightIndex.value = 0;
      isDirty.value = false;
    } else {
      // Default to neutral preset
      applyPreset("neutral");
      isDirty.value = false;
    }
  }

  // Camera framing selection
  function setShot(shot: LightingShot) {
    const s = SHOT_CAMERA_MAP[shot];
    if (!s) return;
    cameraParams.value.shot = shot;
    cameraParams.value.zoom = s.zoom;
    cameraParams.value.focus_y = s.fy;
    cameraParams.value.dof = s.dof;
    isDirty.value = true;
  }

  // Light management
  function addLight(type: LightType): boolean {
    if (lights.value.length >= 8) return false;
    const count = lights.value.length + 1;
    const nameMap: Record<LightType, string> = {
      point: `点光源 #${count}`,
      directional: `平行光 #${count}`,
      spot: `聚光灯 #${count}`,
      ambient: `环境光 #${count}`,
    };
    const template = DEFAULT_NEW_LIGHT[type];
    const newL: LightSource = {
      id: `light-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      name: nameMap[type],
      ...template,
    };
    lights.value.push(newL);
    selectedLightIndex.value = lights.value.length - 1;
    isDirty.value = true;
    return true;
  }

  function removeLight(index: number) {
    if (lights.value.length <= 1) return;
    lights.value.splice(index, 1);
    if (selectedLightIndex.value >= lights.value.length) {
      selectedLightIndex.value = Math.max(0, lights.value.length - 1);
    }
    isDirty.value = true;
  }

  function toggleKeyLight(index: number) {
    lights.value.forEach((l, idx) => {
      l.is_key = idx === index ? !l.is_key : false;
    });
    isDirty.value = true;
  }

  function moveLight(index: number, direction: "up" | "down") {
    const target = direction === "up" ? index - 1 : index + 1;
    if (target < 0 || target >= lights.value.length) return;
    const temp = lights.value[index];
    lights.value[index] = lights.value[target];
    lights.value[target] = temp;
    selectedLightIndex.value = target;
    isDirty.value = true;
  }

  // Export full lighting parameters object matching backend API
  function toLightingParams(legacyBase?: Partial<LightingParams>): LightingParams {
    return {
      character: legacyBase?.character || {
        brightness: 1,
        contrast: 1,
        saturation: 1,
        sepia: 0,
        glow_radius: 0,
        glow_color: "#ffffff",
      },
      background: legacyBase?.background || {
        brightness: 1,
        contrast: 1,
        saturation: 1,
        sepia: 0,
        glow_radius: 0,
        glow_color: "#ffffff",
      },
      overlay_enabled: legacyBase?.overlay_enabled ?? false,
      blend_mode: legacyBase?.blend_mode || "soft-light",
      light_x: legacyBase?.light_x ?? 50,
      light_y: legacyBase?.light_y ?? 50,
      overlay_color1: legacyBase?.overlay_color1 || "#ffd9b0",
      overlay_color2: legacyBase?.overlay_color2 || "#000000",
      overlay_radius: legacyBase?.overlay_radius ?? 50,
      overlay_opacity: legacyBase?.overlay_opacity ?? 0.5,
      overlay_target: legacyBase?.overlay_target || "both",
      lights: JSON.parse(JSON.stringify(lights.value)),
      global: { ...globalParams.value },
      compositing: { ...compositingParams.value },
      material: { ...materialParams.value },
      camera: { ...cameraParams.value },
    };
  }

  // JSON export
  function exportJson(): string {
    const data = {
      version: 4,
      presetId: curPresetId.value,
      camera: cameraParams.value,
      global: globalParams.value,
      compositing: compositingParams.value,
      material: materialParams.value,
      lights: lights.value,
    };
    return JSON.stringify(data, null, 2);
  }

  // JSON import
  function importJson(raw: unknown): boolean {
    if (!raw || typeof raw !== "object") return false;
    const obj = raw as Record<string, unknown>;

    // Case 1: Presets pack array or { presets: [...] }
    const pList = Array.isArray(obj) ? obj : Array.isArray(obj.presets) ? obj.presets : null;
    if (pList) {
      pList.forEach((item) => {
        if (!item || typeof item !== "object" || !item.id) return;
        const target = presets.value.find((p) => p.id === item.id);
        if (target) {
          Object.assign(target, item);
          target._overridden = true;
        }
      });
      applyPreset(curPresetId.value);
      return true;
    }

    // Case 2: Single config
    if (obj.camera && typeof obj.camera === "object") {
      Object.assign(cameraParams.value, obj.camera);
    }
    if (obj.global && typeof obj.global === "object") {
      Object.assign(globalParams.value, obj.global);
    }
    if (obj.compositing && typeof obj.compositing === "object") {
      Object.assign(compositingParams.value, obj.compositing);
    }
    if (obj.material && typeof obj.material === "object") {
      Object.assign(materialParams.value, obj.material);
    }
    if (Array.isArray(obj.lights)) {
      lights.value = (obj.lights as LightSource[]).slice(0, 8);
      selectedLightIndex.value = Math.min(
        selectedLightIndex.value,
        Math.max(0, lights.value.length - 1),
      );
    }
    isDirty.value = true;
    return true;
  }

  return {
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
    isDirty,
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
    commitHistory,
    favoritePresetIds,
    toggleFavoritePreset,
    isPresetFavorite,
  };
}

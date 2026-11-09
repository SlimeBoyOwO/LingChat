import { ref, shallowRef, computed, watch } from "vue";
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
  // shallowRef：快照不该被 Vue 深度代理，否则每次比较/入栈都要遍历一遍响应式对象
  const undoStack = shallowRef<LightingSnapshot[]>([]);
  const redoStack = shallowRef<LightingSnapshot[]>([]);

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

  /**
   * 最近一次已入栈的快照。入栈的是「改动前的状态」，所以 undo 直接取它即可；
   * applySnapshot 触发的 deep watch 会用 sameSnapshot 判等被吞掉，不需要额外的 restoring 标志。
   */
  let current: LightingSnapshot = captureSnapshot();
  let historyTimer: ReturnType<typeof setTimeout> | null = null;

  function sameSnapshot(a: LightingSnapshot, b: LightingSnapshot): boolean {
    return JSON.stringify(a) === JSON.stringify(b);
  }

  function commitHistory() {
    const next = captureSnapshot();
    if (sameSnapshot(next, current)) return;
    undoStack.value = [...undoStack.value, current].slice(-HISTORY_LIMIT);
    redoStack.value = [];
    current = next;
    isDirty.value = true;
  }

  /** 把还在 debounce 窗口里的改动先落定，否则改完 600ms 内按 Ctrl+Z 会把它一起丢掉。 */
  function flushPending() {
    if (!historyTimer) return;
    clearTimeout(historyTimer);
    historyTimer = null;
    commitHistory();
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
    flushPending();
    // 不用 .at()：项目 target lib 低于 ES2022
    const prev = undoStack.value[undoStack.value.length - 1];
    if (!prev) return false;
    undoStack.value = undoStack.value.slice(0, -1);
    redoStack.value = [...redoStack.value, current];
    current = prev;
    applySnapshot(prev);
    return true;
  }

  function redo(): boolean {
    flushPending();
    const next = redoStack.value[redoStack.value.length - 1];
    if (!next) return false;
    redoStack.value = redoStack.value.slice(0, -1);
    undoStack.value = [...undoStack.value, current].slice(-HISTORY_LIMIT);
    current = next;
    applySnapshot(next);
    return true;
  }

  watch(
    [lights, globalParams, compositingParams, materialParams, cameraParams],
    () => {
      if (historyTimer) clearTimeout(historyTimer);
      historyTimer = setTimeout(() => {
        historyTimer = null;
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
    } else {
      // Default to neutral preset
      applyPreset("neutral");
    }

    // 换场景必须清栈并重置基线，否则 Ctrl+Z 会把上一个场景的灯恢复到当前场景上
    if (historyTimer) {
      clearTimeout(historyTimer);
      historyTimer = null;
    }
    undoStack.value = [];
    redoStack.value = [];
    current = captureSnapshot();
    isDirty.value = false;
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
    // 删掉选中项之前的灯时索引要跟着前移，否则选中的会变成下一盏
    if (index < selectedLightIndex.value) selectedLightIndex.value -= 1;
    selectedLightIndex.value = Math.max(
      0,
      Math.min(selectedLightIndex.value, lights.value.length - 1),
    );
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

  // ---- 导入校验 ----
  // 外部 JSON 一律不可信：缺字段会让舞台里的 l.x.toFixed() 抛错，类型错的字段会让
  // Rust 侧反序列化失败并静默丢弃整组 lights（用户看到的是「保存了但不生效」）。

  const LIGHT_TYPES: ReadonlyArray<LightSource["type"]> = [
    "point",
    "directional",
    "spot",
    "ambient",
  ];

  const LIGHT_LIMITS: Record<string, [number, number]> = {
    intensity: [0, 3],
    x: [0, 1],
    y: [0, 1],
    z: [0, 1],
    radius: [0, 1.5],
    softness: [0, 1],
    azimuth: [-180, 180],
    elevation: [0, 90],
    cone: [0, 180],
    penumbra: [0, 1],
  };

  const GROUP_LIMITS: Record<string, Record<string, [number, number]>> = {
    global: {
      master: [0, 2],
      ambient_blend: [0, 1],
      albedo_protect: [0, 1],
      soft_knee: [0, 1],
      lift: [0, 1],
      mono: [0, 1],
    },
    compositing: {
      light_wrap: [0, 1.5],
      rim_width: [0, 40],
      bloom_int: [0, 2.5],
      bloom_thresh: [0.2, 1],
    },
    material: {
      normal_strength: [0, 2],
      cel_thresh: [0, 1],
      cel_softness: [0, 0.5],
      spec_int: [0, 1],
      spec_gloss: [1, 128],
      self_shadow_int: [0, 1],
    },
    camera: {
      zoom: [0.5, 2.5],
      focus_y: [-0.5, 1],
      dof: [0, 1],
    },
  };

  function clampNum(value: unknown, fallback: number, lo: number, hi: number): number {
    const n = typeof value === "number" ? value : Number(value);
    if (!Number.isFinite(n)) return fallback;
    return Math.min(hi, Math.max(lo, n));
  }

  /** 洗一盏灯：缺字段按灯型默认值补齐，数值 clamp，未知灯型直接丢。 */
  function normalizeLight(raw: unknown, index: number): LightSource | null {
    if (!raw || typeof raw !== "object") return null;
    const o = raw as Record<string, unknown>;
    const type = LIGHT_TYPES.find((t) => t === o.type);
    if (!type) return null;

    const base = DEFAULT_NEW_LIGHT[type] as unknown as Record<string, unknown>;
    const out: Record<string, unknown> = {
      id: `light-import-${Date.now().toString(36)}-${index}-${Math.random()
        .toString(36)
        .slice(2, 6)}`,
      name: typeof o.name === "string" && o.name.trim() ? o.name.trim() : `光源 #${index + 1}`,
      type,
      enabled: o.enabled !== false,
      is_key: o.is_key === true,
      color:
        typeof o.color === "string" && /^#[0-9a-fA-F]{6}$/.test(o.color) ? o.color : base.color,
    };
    for (const [key, [lo, hi]] of Object.entries(LIGHT_LIMITS)) {
      out[key] = clampNum(o[key], base[key] as number, lo, hi);
    }
    return out as unknown as LightSource;
  }

  /** 同 normalizeLight，用于 global / compositing / material / camera。 */
  function normalizeGroup<T extends object>(
    raw: unknown,
    defaults: T,
    limits: Record<string, [number, number]>,
  ): T {
    const o = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
    const defs = defaults as unknown as Record<string, unknown>;
    const out: Record<string, unknown> = {};
    for (const [key, def] of Object.entries(defs)) {
      if (typeof def === "number") {
        const [lo, hi] = limits[key] ?? [0, 1];
        out[key] = clampNum(o[key], def, lo, hi);
      } else if (typeof def === "boolean") {
        out[key] = typeof o[key] === "boolean" ? o[key] : def;
      } else {
        out[key] = typeof o[key] === "string" && o[key] ? o[key] : def;
      }
    }
    return out as T;
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
    const cam = normalizeGroup(obj.camera, DEFAULT_CAMERA, GROUP_LIMITS.camera);
    if (!SHOT_CAMERA_MAP[cam.shot]) cam.shot = DEFAULT_CAMERA.shot;
    cameraParams.value = cam;
    globalParams.value = normalizeGroup(obj.global, DEFAULT_GLOBAL, GROUP_LIMITS.global);
    compositingParams.value = normalizeGroup(
      obj.compositing,
      DEFAULT_COMPOSITING,
      GROUP_LIMITS.compositing,
    );
    materialParams.value = normalizeGroup(obj.material, DEFAULT_MATERIAL, GROUP_LIMITS.material);

    if (Array.isArray(obj.lights)) {
      const normalized = obj.lights
        .map((item, i) => normalizeLight(item, i))
        .filter((l): l is LightSource => l !== null)
        .slice(0, 8);
      // 一盏都不合法就整体拒绝，否则会留下「2.5D 路径 + 零盏灯」的空白场景
      if (normalized.length === 0) return false;
      lights.value = normalized;
      selectedLightIndex.value = Math.max(
        0,
        Math.min(selectedLightIndex.value, normalized.length - 1),
      );
    } else {
      // 没带 lights 的 JSON 只改全局参数，不该把现有灯光清空
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

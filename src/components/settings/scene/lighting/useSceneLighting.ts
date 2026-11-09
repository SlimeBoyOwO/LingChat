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
const LS_CUSTOM_KEY = "lingchat.lighting.customPresets";

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

  /**
   * 自定义预设与「内置覆盖」分开存：内置覆盖会被「重置内置」清掉，
   * 自定义是用户自己存出来的资产，不该跟着消失。
   */
  function loadCustomPresets() {
    try {
      const raw = localStorage.getItem(LS_CUSTOM_KEY);
      if (!raw) return;
      const list = JSON.parse(raw);
      if (!Array.isArray(list)) return;
      list.forEach((p) => {
        if (!p || typeof p !== "object" || !p.id) return;
        if (presets.value.some((x) => x.id === p.id)) return;
        presets.value.push({ ...p, builtin: false });
      });
    } catch {
      // 存档坏了就当没有自定义预设
    }
  }
  loadCustomPresets();

  /** 把当前参数快照存成新预设并切过去。 */
  function createCustomPreset(name: string): LightingPreset | null {
    const trimmed = name.trim();
    if (!trimmed) return null;

    const preset: LightingPreset = {
      id: `custom-${Date.now().toString(36)}`,
      name: trimmed,
      cat: "daily",
      desc: "自定义参数快照",
      builtin: false,
      camera: { ...cameraParams.value },
      global: { ...globalParams.value },
      compositing: { ...compositingParams.value },
      material: { ...materialParams.value },
      lights: JSON.parse(JSON.stringify(lights.value)),
      _overridden: true,
    };

    presets.value.push(preset);
    try {
      const raw = localStorage.getItem(LS_CUSTOM_KEY);
      const list = raw ? JSON.parse(raw) : [];
      localStorage.setItem(
        LS_CUSTOM_KEY,
        JSON.stringify(Array.isArray(list) ? [...list, preset] : [preset]),
      );
    } catch {
      // 存不进 localStorage 也让它在本会话里可用
    }
    applyPreset(preset.id);
    return preset;
  }

  const selectedLight = computed<LightSource | null>(() => {
    if (selectedLightIndex.value >= 0 && selectedLightIndex.value < lights.value.length) {
      return lights.value[selectedLightIndex.value];
    }
    return null;
  });

  const currentPreset = computed(() => presets.value.find((p) => p.id === curPresetId.value));

  /**
   * 预设的第一盏灯基本都是环境底光。编辑面板默认选中真正的发光灯（点光/主光），
   * 否则一打开右栏全是「环境光」的三条参数，连位置滑块都没有。
   */
  function defaultSelectedIndex(): number {
    const i = lights.value.findIndex((l) => l.type !== "ambient");
    return i >= 0 ? i : 0;
  }

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

    selectedLightIndex.value = defaultSelectedIndex();
    isDirty.value = true;
  }
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
    // 零灯是非法状态：场景会走 WebGL 路径却没有任何光源，画面被压成一片黑。
    // 这类数据只可能来自旧版本或导入了坏的 JSON，一律回落到 neutral 预设。
    const incoming = initial?.lights;
    if (Array.isArray(incoming) && incoming.length > 0) {
      lights.value = JSON.parse(JSON.stringify(incoming)).slice(0, 8);
      if (initial?.global) globalParams.value = { ...initial.global };
      if (initial?.compositing) compositingParams.value = { ...initial.compositing };
      if (initial?.material) materialParams.value = { ...initial.material };
      if (initial?.camera) cameraParams.value = { ...initial.camera };
      selectedLightIndex.value = defaultSelectedIndex();
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
  /** 新增一盏灯；`pos` 用于「点画布放置光源」时直接落在点击位置。 */
  function addLight(type: LightType, pos?: { x: number; y: number }): boolean {
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
    if (pos) {
      newL.x = Math.min(1, Math.max(0, +pos.x.toFixed(3)));
      newL.y = Math.min(1, Math.max(0, +pos.y.toFixed(3)));
    }
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

  // ---- JSON 导入（先校验、后落地）----
  // 外部 JSON 一律不可信。原则：**校验全部通过前不碰现有状态**，
  // 否则一份坏数据会把面板改成半新半旧，用户只能重开面板。

  const PRESET_CATS: ReadonlyArray<string> = ["daily", "atmosphere", "drama", "dark"];

  /** 洗一条预设：id/name 必须存在，灯光至少一盏合法，其余分组按默认值补齐。 */
  function sanitizePreset(raw: unknown): LightingPreset | null {
    if (!raw || typeof raw !== "object") return null;
    const o = raw as Record<string, unknown>;
    const id = typeof o.id === "string" ? o.id.trim() : "";
    const name = typeof o.name === "string" ? o.name.trim() : "";
    if (!id || !name) return null;

    const lightsRaw = Array.isArray(o.lights) ? o.lights : [];
    const cleaned = lightsRaw
      .map((item, i) => normalizeLight(item, i))
      .filter((l): l is LightSource => l !== null)
      .slice(0, 8);
    if (cleaned.length === 0) return null;

    const cam = normalizeGroup(o.camera, DEFAULT_CAMERA, GROUP_LIMITS.camera);
    if (!SHOT_CAMERA_MAP[cam.shot]) cam.shot = DEFAULT_CAMERA.shot;
    const cat = typeof o.cat === "string" && PRESET_CATS.includes(o.cat) ? o.cat : "daily";

    return {
      id,
      name,
      cat: cat as LightingPreset["cat"],
      desc: typeof o.desc === "string" && o.desc.trim() ? o.desc.trim() : "导入的预设",
      builtin: false,
      bgKey: typeof o.bgKey === "string" ? o.bgKey : undefined,
      shotKey: cam.shot,
      camera: cam,
      global: normalizeGroup(o.global, DEFAULT_GLOBAL, GROUP_LIMITS.global),
      compositing: normalizeGroup(o.compositing, DEFAULT_COMPOSITING, GROUP_LIMITS.compositing),
      material: normalizeGroup(o.material, DEFAULT_MATERIAL, GROUP_LIMITS.material),
      lights: cleaned,
      _overridden: true,
    };
  }

  /**
   * 导入一份 JSON。
   *
   * 两种形状：预设包（数组 / `{presets:[]}`）或单份配置。
   * 任一形状都先完整校验，全部合法才写状态；返回可读回执给 UI 弹提示。
   */
  function importJson(raw: unknown): { ok: boolean; message: string } {
    if (!raw || typeof raw !== "object") {
      return { ok: false, message: "JSON 不是一个对象" };
    }
    const obj = raw as Record<string, unknown>;

    // —— Case 1：预设包 ——
    const pList = Array.isArray(obj) ? obj : Array.isArray(obj.presets) ? obj.presets : null;
    if (pList) {
      const cleaned: LightingPreset[] = [];
      let skipped = 0;
      for (const item of pList.slice(0, 200)) {
        const p = sanitizePreset(item);
        if (p) cleaned.push(p);
        else skipped++;
      }
      if (!cleaned.length) {
        return { ok: false, message: "预设包里没有一条合法预设（缺 id/name 或灯光全非法）" };
      }
      cleaned.forEach((p) => {
        const target = presets.value.find((x) => x.id === p.id);
        if (target) Object.assign(target, p);
        else presets.value.push(p);
      });
      applyPreset(cleaned[0].id);
      const tail = skipped ? `，跳过 ${skipped} 条非法` : "";
      return { ok: true, message: `已导入 ${cleaned.length} 条预设${tail}` };
    }

    // —— Case 2：单份配置 ——
    // 先算好全部候选值，不写状态
    const cam = normalizeGroup(obj.camera, DEFAULT_CAMERA, GROUP_LIMITS.camera);
    if (!SHOT_CAMERA_MAP[cam.shot]) cam.shot = DEFAULT_CAMERA.shot;
    const nextGlobal = normalizeGroup(obj.global, DEFAULT_GLOBAL, GROUP_LIMITS.global);
    const nextCompositing = normalizeGroup(
      obj.compositing,
      DEFAULT_COMPOSITING,
      GROUP_LIMITS.compositing,
    );
    const nextMaterial = normalizeGroup(obj.material, DEFAULT_MATERIAL, GROUP_LIMITS.material);

    let nextLights: LightSource[] | null = null;
    let droppedLights = 0;
    if (Array.isArray(obj.lights)) {
      const normalized = obj.lights
        .map((item, i) => normalizeLight(item, i))
        .filter((l): l is LightSource => l !== null)
        .slice(0, 8);
      droppedLights = Math.max(0, obj.lights.length - normalized.length);
      if (normalized.length === 0) {
        // 一盏都不合法就整体拒绝，否则会留下「2.5D 路径 + 零盏灯」的空白场景
        return { ok: false, message: "灯光数据全部非法，已拒绝导入" };
      }
      nextLights = normalized;
    }

    // 到这里才提交
    cameraParams.value = cam;
    globalParams.value = nextGlobal;
    compositingParams.value = nextCompositing;
    materialParams.value = nextMaterial;
    if (nextLights) {
      lights.value = nextLights;
      selectedLightIndex.value = Math.max(
        0,
        Math.min(selectedLightIndex.value, nextLights.length - 1),
      );
    }
    isDirty.value = true;

    const parts = ["已应用配置"];
    if (nextLights) parts.push(`${nextLights.length} 盏灯`);
    if (droppedLights) parts.push(`跳过 ${droppedLights} 盏非法灯`);
    return { ok: true, message: parts.join(" · ") };
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
    createCustomPreset,
  };
}

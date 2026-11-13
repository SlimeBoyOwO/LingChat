import { ref, shallowRef, computed, watch } from "vue";
import type {
  LightSource,
  LightType,
  LightingCameraParams,
  LightingCompositingParams,
  LightingGlobalParams,
  LightingMaterialParams,
  LightingParams,
  LightingSet,
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
// 灯数上限与渲染打包共用同一个常量，否则存档与预览的截断口径会各走一套
import { MAX_LIGHTS } from "@/render/lighting/lights";

const LS_OVERRIDE_KEY = "lingchat.lighting.builtinOverrides";
const LS_FAV_KEY = "lingchat.lighting.favoritePresets";
const LS_CUSTOM_KEY = "lingchat.lighting.customPresets";

// ---- 导入校验的量程表 ----
// 外部 JSON 一律不可信：缺字段会让舞台里的 l.x.toFixed() 抛错，类型错的字段会让
// Rust 侧反序列化失败并静默丢弃整组 lights（用户看到的是「保存了但不生效」）。
//
// 这几个表放在模块级而不是 composable 里：`loadCustomPresets()` 在 composable 体内
// 就要执行，而它依赖 `sanitizePreset` → 依赖这些常量，同作用域后置声明会撞 TDZ。

const LIGHT_TYPES: ReadonlyArray<LightSource["type"]> = ["point", "directional", "spot", "ambient"];

const PRESET_CATS: ReadonlyArray<string> = ["daily", "atmosphere", "drama", "dark"];

/**
 * 量程的唯一口径：UI 滑块范围（workbench 里的 ParamSlider）与 `api/services/scene.ts`
 * 的字段注释。两者取并集，且必须容纳 `DEFAULT_NEW_LIGHT` 与 `presetsData` 的校准值 ——
 * 校验表比它们紧，导出再导入就会把校准预设悄悄改掉（方位角 335 被夹成 180 那类）。
 */
const LIGHT_LIMITS: Record<string, [number, number]> = {
  intensity: [0, 3],
  x: [0, 1],
  y: [0, 1],
  z: [0, 1.5],
  radius: [0, 2.2],
  softness: [0, 1],
  azimuth: [0, 360],
  elevation: [-10, 90],
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
    saturation: [0, 2],
    contrast: [0, 2],
    tint_hue: [-180, 180],
    tint_amount: [0, 1],
    char_exposure: [0, 2],
    bg_exposure: [0, 2],
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
    spec_int: [0, 1.5],
    spec_gloss: [1, 128],
    self_shadow_int: [0, 1],
  },
  camera: {
    zoom: [0.5, 2.5],
    focus_y: [-0.5, 1],
    // 渲染器按 dof / 6 归一化模糊半径，校准预设最高用到 4.5
    dof: [0, 6],
  },
};

interface LightingSnapshot {
  lights: LightSource[];
  global: LightingGlobalParams;
  compositing: LightingCompositingParams;
  material: LightingMaterialParams;
  camera: LightingCameraParams;
  selectedLightIndex: number;
  curPresetId: string;
}

/**
 * loadSet 的入参形状：场景那份 LightingParams 与角色那份 LightingSet 的五组字段同名，
 * 只是可空性不同，取宽的那一边两边就都能传。
 */
interface LightingSetLike {
  lights?: LightSource[] | null;
  global?: LightingGlobalParams | null;
  compositing?: LightingCompositingParams | null;
  material?: LightingMaterialParams | null;
  camera?: LightingCameraParams | null;
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
        // localStorage 和外部 JSON 一样不可信：手改过或旧版本留下的坏数据不洗就进状态，
        // 等于从旁边绕过了整套导入校验（灯数超限、字段类型错）
        const clean = sanitizePreset(p);
        if (!clean) return;
        if (presets.value.some((x) => x.id === clean.id)) return;
        presets.value.push(clean);
      });
    } catch {
      // 存档坏了就当没有自定义预设
    }
  }
  loadCustomPresets();

  /** 按 id 合并写入自定义预设存档。写不进 localStorage 也不影响本会话可用。 */
  function persistCustomPresets(incoming: LightingPreset[]) {
    if (!incoming.length) return;
    try {
      const raw = localStorage.getItem(LS_CUSTOM_KEY);
      const parsed = raw ? JSON.parse(raw) : [];
      const merged: unknown[] = Array.isArray(parsed) ? parsed : [];
      for (const p of incoming) {
        const i = merged.findIndex((x) => x && (x as LightingPreset).id === p.id);
        if (i >= 0) merged[i] = p;
        else merged.push(p);
      }
      localStorage.setItem(LS_CUSTOM_KEY, JSON.stringify(merged));
    } catch {
      // 存不进就算了
    }
  }

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
    persistCustomPresets([preset]);
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
    lights.value = normalizeLights(JSON.parse(JSON.stringify(p.lights)));

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
    return persistOverridePatch(cur.id, patch);
  }

  /** 内置预设的改动只存 patch，与 `loadOverrides` 的 `Object.assign` 口径一致。 */
  function persistOverridePatch(id: string, patch: Partial<LightingPreset>): boolean {
    try {
      const stored = localStorage.getItem(LS_OVERRIDE_KEY);
      const overrides = stored ? JSON.parse(stored) : {};
      overrides[id] = patch;
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

  /**
   * 载入一套光影并把它当成新的基线。
   *
   * 打开工作台（场景那份 LightingParams）与切换编辑目标（某个角色的 LightingSet）
   * 走的是同一条路：形状不同但五组字段同名，所以入参放宽成 LightingSetLike。
   */
  function loadSet(incoming: LightingSetLike | null | undefined) {
    // 零灯是非法状态：场景会走 WebGL 路径却没有任何光源，画面被压成一片黑。
    // 这类数据只可能来自旧版本或导入了坏的 JSON，一律回落到 neutral 预设。
    const cleaned = Array.isArray(incoming?.lights)
      ? normalizeLights(JSON.parse(JSON.stringify(incoming?.lights)))
      : [];
    if (cleaned.length > 0) {
      lights.value = cleaned;
      // 缺某个可选分组时回落默认值。写成 `if (initial?.global)` 的话，缺组的场景会沿用
      // 上一个场景残留的参数 —— 面板是复用的，状态不会自己清，看着就像「串场景」。
      // 这里只补缺、不夹数值：存档里的值可能来自校准预设，夹一次就变脸。
      globalParams.value = { ...DEFAULT_GLOBAL, ...incoming?.global };
      compositingParams.value = { ...DEFAULT_COMPOSITING, ...incoming?.compositing };
      materialParams.value = { ...DEFAULT_MATERIAL, ...incoming?.material };
      cameraParams.value = { ...DEFAULT_CAMERA, ...incoming?.camera };
      selectedLightIndex.value = defaultSelectedIndex();
    } else {
      // Default to neutral preset
      applyPreset("neutral");
    }

    // 换场景 / 换编辑目标都必须清栈并重置基线，否则 Ctrl+Z 会把上一份的灯恢复到当前这份上
    if (historyTimer) {
      clearTimeout(historyTimer);
      historyTimer = null;
    }
    undoStack.value = [];
    redoStack.value = [];
    current = captureSnapshot();
    isDirty.value = false;
  }

  /** 当前文档态的那一套，供切换编辑目标时整存整取。灯过一遍闸门，与保存口径一致。 */
  function exportSet(): LightingSet {
    return {
      lights: normalizeLights(JSON.parse(JSON.stringify(lights.value))),
      global: { ...globalParams.value },
      compositing: { ...compositingParams.value },
      material: { ...materialParams.value },
      camera: { ...cameraParams.value },
    };
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
    if (lights.value.length >= MAX_LIGHTS) return false;
    const count = lights.value.length + 1;
    const nameMap: Record<LightType, string> = {
      point: `点光源 #${count}`,
      directional: `平行光 #${count}`,
      spot: `聚光灯 #${count}`,
      ambient: `环境光 #${count}`,
    };
    const template = DEFAULT_NEW_LIGHT[type];
    const newL: LightSource = {
      id: newLightId(lights.value.length),
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
  /**
   * 拼出存档用的 LightingParams。
   *
   * `set` 用于「当前文档态不是场景那套」的场合（正在编辑某个角色的专属光影），
   * 不传就取文档态。旧滤镜字段一律沿用 legacyBase，避免高级面板把它们算回默认。
   */
  function toLightingParams(
    legacyBase?: Partial<LightingParams>,
    set?: LightingSet,
  ): LightingParams {
    const doc = set ?? exportSet();
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
      // 保存边界也过一遍闸门：渲染打包会截断到 MAX_LIGHTS，存档不截断就会留下
      // 「在存档里、却永远不参与预览」的幽灵灯
      lights: doc.lights,
      global: doc.global,
      compositing: doc.compositing,
      material: doc.material,
      camera: doc.camera,
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

  function clampNum(value: unknown, fallback: number, lo: number, hi: number): number {
    const n = typeof value === "number" ? value : Number(value);
    if (!Number.isFinite(n)) return fallback;
    return Math.min(hi, Math.max(lo, n));
  }

  function newLightId(index: number): string {
    return `light-${Date.now().toString(36)}-${index}-${Math.random().toString(36).slice(2, 6)}`;
  }

  /** 洗一盏灯：缺字段按灯型默认值补齐，数值 clamp，未知灯型直接丢。 */
  function normalizeLight(raw: unknown, index: number): LightSource | null {
    if (!raw || typeof raw !== "object") return null;
    const o = raw as Record<string, unknown>;
    const type = LIGHT_TYPES.find((t) => t === o.type);
    if (!type) return null;

    const base = DEFAULT_NEW_LIGHT[type] as unknown as Record<string, unknown>;
    const out: Record<string, unknown> = {
      // 已有合法 id 就留着：拖拽、选中、撤销都按 id 认灯，每次导入都换 id 会断掉这些关联
      id: typeof o.id === "string" && o.id.trim() ? o.id.trim() : newLightId(index),
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

  /**
   * 灯数与结构的统一闸门：初始化、预设应用、JSON 导入、最终保存四个边界都过一遍。
   *
   * 只管结构与数量，不碰数值 —— 校准预设的数值一旦被夹，导出再导入就变了脸。
   * 数值合法性是 `normalizeLight` 的活，只在数据不可信（外部 JSON / localStorage）时走。
   */
  function normalizeLights(list: LightSource[]): LightSource[] {
    const out: LightSource[] = [];
    const seen = new Set<string>();
    for (const l of list) {
      if (!l || typeof l !== "object") continue;
      if (!LIGHT_TYPES.includes(l.type)) continue;
      let id = typeof l.id === "string" && l.id.trim() ? l.id.trim() : "";
      if (!id || seen.has(id)) id = newLightId(out.length);
      seen.add(id);
      out.push({ ...l, id });
      if (out.length >= MAX_LIGHTS) break;
    }
    return out;
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
  // 外部 JSON 一律不可信。原则：校验全部通过前不碰现有状态，
  // 否则一份坏数据会把面板改成半新半旧，用户只能重开面板。

  /** 洗一条预设：id/name 必须存在，灯光至少一盏合法，其余分组按默认值补齐。 */
  function sanitizePreset(raw: unknown): LightingPreset | null {
    if (!raw || typeof raw !== "object") return null;
    const o = raw as Record<string, unknown>;
    const id = typeof o.id === "string" ? o.id.trim() : "";
    const name = typeof o.name === "string" ? o.name.trim() : "";
    if (!id || !name) return null;

    const lightsRaw = Array.isArray(o.lights) ? o.lights : [];
    const cleaned = normalizeLights(
      lightsRaw
        .map((item, i) => normalizeLight(item, i))
        .filter((l): l is LightSource => l !== null),
    );
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
      // 导入的预设包要落盘，否则刷新即丢。撞上内置 id 的走 builtinOverrides
      // （与「覆盖当前预设」同一条路），新 id 走自定义存档。
      const customs: LightingPreset[] = [];
      cleaned.forEach((p) => {
        const target = presets.value.find((x) => x.id === p.id);
        // 保留 builtin 标记：洗过的预设一律带 builtin:false，覆盖上去会让内置预设变得可删
        if (target) Object.assign(target, p, { builtin: target.builtin });
        else presets.value.push(p);
        if (BUILTIN_PRESETS.some((b) => b.id === p.id)) {
          persistOverridePatch(p.id, {
            camera: p.camera,
            global: p.global,
            compositing: p.compositing,
            material: p.material,
            lights: p.lights,
          });
        } else {
          customs.push(p);
        }
      });
      persistCustomPresets(customs);
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
    let cappedLights = 0;
    if (Array.isArray(obj.lights)) {
      const valid = obj.lights
        .map((item, i) => normalizeLight(item, i))
        .filter((l): l is LightSource => l !== null);
      const normalized = normalizeLights(valid);
      droppedLights = obj.lights.length - valid.length;
      cappedLights = valid.length - normalized.length;
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
    if (cappedLights) parts.push(`超出 ${MAX_LIGHTS} 盏上限，截断 ${cappedLights} 盏`);
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
    loadSet,
    exportSet,
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

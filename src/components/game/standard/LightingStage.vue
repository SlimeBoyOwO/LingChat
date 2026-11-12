<template>
  <!--
    2.5D 光影舞台。

    场景带 `lighting.lights` 时，用 WebGL 逐像素重画背景，并让 GameBackground
    让出它那一层。

    角色分两种：
      - 静态立绘（avatar_mode: image）：由 `useStaticAvatarTakeover` 接管 —— 读 DOM
        里那个 <img> 的矩形、灌进渲染器、把 DOM 图藏起来，光照才能真正打在人物上。
      - Live2D：拿不到每帧位图，只能打背景，立绘仍由 Live2DRolePresentation 渲染。

    没有灯光的场景完全不启用，旧路径（CSS 滤镜 + 径向渐变）照旧。
  -->
  <canvas
    v-show="enabled"
    ref="canvasRef"
    :key="canvasKey"
    class="lighting-stage"
    aria-hidden="true"
  ></canvas>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { resolveBackgroundSrc } from "@/utils/background-src";
import {
  SceneLightingRenderer,
  generatePseudoDepth,
  generatePseudoNormal,
} from "@/render/lighting";
import { loadTexImage, type LoadedImage } from "@/render/lighting/loadImage";
import { useStaticAvatarTakeover } from "@/composables/game/useStaticAvatarTakeover";
import {
  acquireLightingStage,
  releaseLightingStage,
} from "@/composables/game/useSceneLightingStage";

const gameStore = useGameStore();
const uiStore = useUIStore();

const canvasRef = ref<HTMLCanvasElement | null>(null);
/**
 * 画布代号。`renderer.dispose()` 会主动 loseContext 还掉上下文（否则反复开关
 * 工作台会累积到 Chromium 的 ~16 个上限），而丢过上下文的 canvas 再取还是那个
 * 死上下文：shader 编译静默失败，舞台再也起不来。这里每次拆台就换一块新 canvas。
 */
const canvasKey = ref(0);
const enabled = ref(false);

const lighting = computed(() => gameStore.currentScene?.lighting ?? null);
const hasLights = computed(() => (lighting.value?.lights?.length ?? 0) > 0);

/**
 * 旧的 overlay 径向渐变异色层。字段一直在 LightingParams 里，只是 2.5D 路径
 * 之前没消费它 —— 不接的话，开了工作台的场景会丢掉原来那层双色光晕。
 */
const overlayParams = computed(() => {
  const l = lighting.value;
  if (!l) return null;
  return {
    enabled: !!l.overlay_enabled,
    color1: l.overlay_color1 || "#ffd9b0",
    color2: l.overlay_color2 || "#000000",
    radius: l.overlay_radius ?? 50,
    opacity: l.overlay_opacity ?? 0.5,
    center: [(l.light_x ?? 50) / 100, (l.light_y ?? 50) / 100] as [number, number],
    mode: l.blend_mode || "normal",
  };
});
const backgroundSrc = computed(() => resolveBackgroundSrc(uiStore.currentBackground));

/**
 * 立绘法线/深度缓存。立绘是 2304×3072，一次生成约 300ms；表情切换很频繁，
 * 不缓存的话每换一次脸就卡一下。按图片 URL 存，最多留 8 张。
 */
const mapCache = new Map<string, { normal: HTMLCanvasElement; depth: HTMLCanvasElement }>();
const MAP_CACHE_MAX = 8;

/** 伪法线长边上限，与渲染器内部一致。 */
const NORMAL_MAX_DIM = 768;
/** 伪深度长边上限，与渲染器内部一致。 */
const DEPTH_MAX_DIM = 512;

const takeover = useStaticAvatarTakeover({
  enabled: () => enabled.value && hasLights.value,
  canvas: () => canvasRef.value,
  roleId: () => gameStore.currentInteractRoleId ?? null,
  onStatus: (status, detail) => {
    if (status === "active") console.info("[lighting] 静态立绘已接管", detail ?? "");
    else if (status === "fail")
      console.warn("[lighting] 立绘接管失败，保持 DOM 渲染：", detail ?? "");
    else console.info("[lighting] 立绘未接管", detail ?? "");
  },
});

/** 生成（或从缓存取）法线/深度，避免每次换表情都重算。 */
function mapsFor(src: string, loaded: LoadedImage) {
  const cached = mapCache.get(src);
  if (cached) return cached;

  const longest = Math.max(loaded.width, loaded.height) || 1;
  const scale = longest > DEPTH_MAX_DIM ? DEPTH_MAX_DIM / longest : 1;
  const depth = generatePseudoDepth(
    loaded.source,
    Math.max(1, Math.round(loaded.width * scale)),
    Math.max(1, Math.round(loaded.height * scale)),
  );
  const normal = generatePseudoNormal(loaded.source, NORMAL_MAX_DIM);

  if (mapCache.size >= MAP_CACHE_MAX) {
    const oldest = mapCache.keys().next().value;
    if (oldest) mapCache.delete(oldest);
  }
  const entry = { normal, depth };
  mapCache.set(src, entry);
  return entry;
}

let renderer: SceneLightingRenderer | null = null;
let background: { source: TexImageSource; width: number; height: number } | null = null;
let observer: ResizeObserver | null = null;
let acquired = false;
let loadToken = 0;
/** start() 会被背景到位、参数变化等多条路触发，用旗标挡住并发重入 */
let starting = false;
let retryTimers: number[] = [];

/**
 * 按设备像素比取画布尺寸。
 *
 * 必须交给 `renderer.resize()`：离屏目标（背景 / 场景 / 模糊 / 泛光）都是在
 * 它里面分配的。只写 `canvas.width/height` 的话 `bgTarget` 一直是 null，
 * `render()` 会静默 return —— 画布保持黑色，看着就像"光影没生效"。
 */
function syncCanvasSize(): { width: number; height: number } {
  const canvas = canvasRef.value;
  if (!canvas) return { width: 0, height: 0 };
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const width = Math.max(1, Math.round(canvas.clientWidth * dpr));
  const height = Math.max(1, Math.round(canvas.clientHeight * dpr));
  renderer?.resize(width, height);
  return { width, height };
}

/** 实际重绘次数，用于离线量「呼吸动画会不会把整条管线跑满」 */
let drawCount = 0;

function draw(): void {
  if (!renderer || !background || !canvasRef.value) return;
  drawCount += 1;
  const { width, height } = syncCanvasSize();
  if (width <= 1 || height <= 1) return;

  // 背景按 cover 铺满舞台，与 ImageAcrossFade 的 object-fit: cover 等价
  const scale = Math.max(width / background.width, height / background.height);
  const bgWidth = background.width * scale;
  const bgHeight = background.height * scale;
  const bgRect: [number, number, number, number] = [
    (width - bgWidth) / 2,
    (height - bgHeight) / 2,
    bgWidth,
    bgHeight,
  ];

  renderer.render({
    lights: lighting.value?.lights ?? [],
    global: lighting.value?.global ?? null,
    compositing: lighting.value?.compositing ?? null,
    material: lighting.value?.material ?? null,
    camera: lighting.value?.camera ?? null,
    overlay: overlayParams.value,
    stage: { width, height },
    backgroundRect: bgRect,
    // 零尺寸矩形 = 着色器走「纯背景」分支（Live2D、接管失败、或还没量到）
    characterRect:
      takeover.active.value && takeover.rect.value ? takeover.rect.value : [0, 0, 0, 0],
  });
}

function clearRetries(): void {
  for (const t of retryTimers) window.clearTimeout(t);
  retryTimers = [];
}

/**
 * 接管首次失败多半是暂时的：立绘还在解码、或盒子还没布局（clientWidth 为 0）。
 * 失败后 ticker 不会启动，没人再补 —— 整场对话立绘都不吃光。这里按退避补三次。
 */
function retryTakeover(): void {
  clearRetries();
  let delay = 200;
  for (let i = 0; i < 3; i++) {
    const wait = delay;
    retryTimers.push(
      window.setTimeout(() => {
        if (!renderer || takeover.active.value) return;
        void takeover.sync();
      }, wait),
    );
    delay *= 3;
  }
}

function teardown(): void {
  // 顺序要紧：先把立绘还给 DOM，再拆渲染器 —— 否则中途失败时人物会消失
  takeover.release();
  clearRetries();
  if (acquired) {
    releaseLightingStage();
    acquired = false;
  }
  enabled.value = false;
  observer?.disconnect();
  observer = null;
  renderer?.dispose();
  renderer = null;
  background = null;
  // 换一块干净画布：这块的上下文已经随 dispose 丢掉了，复用只会静默失败
  canvasKey.value += 1;
}

async function applyBackground(src: string): Promise<void> {
  if (!src) {
    // 让位给了 WebGL、却连背景图都没有 → GameBackground 已经隐藏，会露出一块空白
    if (renderer) console.info("[lighting] 没有背景图，回退到 CSS 光影");
    teardown();
    return;
  }
  const token = ++loadToken;
  const loaded = await loadTexImage(src);
  // 期间又换了背景或实例已销毁，丢弃这次结果
  if (token !== loadToken) return;
  if (!loaded || loaded.tainted) {
    // 背景图拿不到（协议不支持 / 文件缺失 / 跨源污染）时让出背景层，
    // 宁可退回 CSS 路径也不要留一块黑板
    console.warn(
      "[lighting] 背景加载失败，回退到 CSS 光影",
      src,
      loaded ? "图片会污染画布，WebGL 不能上传" : "取不到位图",
    );
    teardown();
    return;
  }
  if (!renderer) {
    // 舞台还没起来（或上一次失败后拆掉了）：借这次背景到位重新拉起。
    // 少了这条，背景晚于场景到位时舞台会永久停在 CSS 路径，之后调光影全无反应。
    await start();
    return;
  }
  background = { source: loaded.source, width: loaded.width, height: loaded.height };
  renderer.setBackground(loaded.source);
  draw();
}

/** 起舞台的门槛检查 + 并发闸；真正的启动流程在 startStage。 */
async function start(): Promise<void> {
  if (starting) return;
  const canvas = canvasRef.value;
  if (!canvas) return;
  if (!hasLights.value) return;
  if (!SceneLightingRenderer.isSupported()) {
    // 这里以前是静默 return：控制台一句话都没有，看着就是「光影没生效」却无从查起
    console.info("[lighting] 当前环境不支持 WebGL，走 CSS 光影");
    return;
  }
  starting = true;
  try {
    await startStage();
  } finally {
    starting = false;
  }
}

async function startStage(): Promise<void> {
  // 先让画布可见，否则 display:none 下 clientWidth 为 0，尺寸会被量成 1x1
  enabled.value = true;
  await nextTick();
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

  // 画布要等 nextTick 之后再取：teardown 会按 :key 换一块新的，
  // 拿旧的那块等于拿一块已经 loseContext 的死上下文
  const canvas = canvasRef.value;
  if (!canvas) return;

  try {
    renderer = new SceneLightingRenderer(canvas);
  } catch (err) {
    // WebGL 不可用时退回旧路径，不让整屏背景因为光影挂掉
    console.warn("[lighting] WebGL 初始化失败，回退到 CSS 光影", err);
    teardown();
    return;
  }

  syncCanvasSize();

  observer?.disconnect();
  observer = new ResizeObserver(() => draw());
  observer.observe(canvas);

  if (!acquired) {
    acquireLightingStage();
    acquired = true;
  }

  // immediate 的 watch 在 renderer 就绪前就跑过了，这里必须补一次
  await applyBackground(backgroundSrc.value);
  if (!renderer) return;

  // 自检：贴图没真正就绪就把背景层还回去。
  // 只置 enabled 不管后果的话，GameBackground 已经隐藏，用户看到的是一片空白。
  if (!renderer.backgroundReady) {
    console.warn("[lighting] 背景贴图未就绪，回退到 CSS 光影：", renderer.lastError);
    teardown();
    return;
  }

  // 背景就绪后再试接管立绘；失败只是「人物不吃光」，不影响背景
  await takeover.sync();
  if (!takeover.active.value) retryTakeover();

  // 一行说清当前状态，方便在控制台一眼判断卡在哪一环
  const stage = syncCanvasSize();
  console.info(
    `[lighting] WebGL 生效：场景「${gameStore.currentScene?.scene_name ?? "?"}」` +
      `${lighting.value?.lights?.length ?? 0} 盏灯 · ${stage.width}x${stage.height} · ` +
      `立绘${takeover.active.value ? "已接管" : "未接管（仍走 DOM）"}`,
  );
  draw();
}

watch(backgroundSrc, (src) => void applyBackground(src));

// 说话人切换时重新挑接管对象。WebGL 只有一个角色纹理槽，不重挑就会一直照旧角色、
// 新说话人反而没光。新立绘可能还在解码，解码完再补一次接管。
watch(
  () => gameStore.currentInteractRoleId,
  () => {
    if (!(enabled.value && hasLights.value)) return;
    void takeover.sync();
    const el = document.querySelector<HTMLElement>(
      `[data-lighting-avatar][data-role-id="${String(gameStore.currentInteractRoleId ?? "")}"]`,
    );
    for (const img of el?.querySelectorAll("img") ?? []) {
      if (!img.complete) img.addEventListener("load", () => void takeover.sync(), { once: true });
    }
  },
);

watch(
  hasLights,
  async (has) => {
    if (has) {
      await start();
      return;
    }
    if (renderer) {
      console.info(
        "[lighting] 当前场景没有多光源数据，走 CSS 路径：",
        gameStore.currentScene?.scene_name ?? "(无场景)",
      );
    }
    teardown();
  },
  { immediate: true },
);

// 灯位/强度变化时重绘；不需要 rAF，场景参数不是逐帧动画。
// 舞台已经退回 CSS 时（renderer 为 null）改参数要能把它重新拉起来，
// 否则「先失败一次、之后怎么调都没反应」。
watch(
  lighting,
  () => {
    if (renderer) draw();
    else void start();
  },
  { deep: true },
);

// 接管到的立绘换了图（表情/换装）→ 灌纹理。缓存命中时是同步的
watch(takeover.image, (loaded) => {
  if (!renderer || !loaded) return;
  const src = takeover.source.value;
  try {
    const maps = mapsFor(src, loaded);
    renderer.setCharacter(loaded.source, maps);
  } catch (err) {
    // 生成失败就交给渲染器自己算 auto，再不行它会回退平法线
    console.warn("[lighting] 立绘贴图准备失败，交回渲染器处理:", err);
    renderer.setCharacter(loaded.source, { normal: "auto", depth: "auto" });
  }
  draw();
});

// 呼吸动画会让矩形缓慢变化，只在真的动了之后重绘
watch(takeover.rect, () => draw());

onMounted(() => {
  if (hasLights.value) void start();
});

onBeforeUnmount(() => {
  loadToken += 1;
  teardown();
});

// 离线 harness / 排障用：把接管状态暴露出去，便于量矩形、看渲染器状态
defineExpose({
  takeover,
  debug: () => ({
    hasRenderer: !!renderer,
    backgroundReady: renderer?.backgroundReady ?? null,
    characterReady: renderer?.characterReady ?? null,
    lastError: renderer?.lastError ?? "",
    charRect: takeover.rect.value,
    active: takeover.active.value,
    stage: canvasRef.value ? { w: canvasRef.value.width, h: canvasRef.value.height } : null,
    drawCount,
  }),
});
</script>

<style scoped>
.lighting-stage {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  z-index: 1;
  pointer-events: none;
}
</style>

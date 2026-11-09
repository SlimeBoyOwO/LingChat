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
  <canvas v-show="enabled" ref="canvasRef" class="lighting-stage" aria-hidden="true"></canvas>
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
import { useStaticAvatarTakeover } from "@/composables/game/useStaticAvatarTakeover";
import {
  acquireLightingStage,
  releaseLightingStage,
} from "@/composables/game/useSceneLightingStage";

const gameStore = useGameStore();
const uiStore = useUIStore();

const canvasRef = ref<HTMLCanvasElement | null>(null);
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
  scale: () => {
    const canvas = canvasRef.value;
    if (!canvas || !canvas.clientWidth) return 1;
    return canvas.width / canvas.clientWidth;
  },
  roleId: () => gameStore.currentInteractRoleId ?? null,
  onStatus: (status, detail) => {
    if (status === "active") console.info("[lighting] 静态立绘已接管", detail ?? "");
    else if (status === "fail")
      console.warn("[lighting] 立绘接管失败，保持 DOM 渲染：", detail ?? "");
    else console.info("[lighting] 立绘未接管", detail ?? "");
  },
});

/** 生成（或从缓存取）法线/深度，避免每次换表情都重算。 */
function mapsFor(src: string, img: HTMLImageElement) {
  const cached = mapCache.get(src);
  if (cached) return cached;

  const longest = Math.max(img.naturalWidth, img.naturalHeight) || 1;
  const scale = longest > DEPTH_MAX_DIM ? DEPTH_MAX_DIM / longest : 1;
  const depth = generatePseudoDepth(
    img,
    Math.max(1, Math.round(img.naturalWidth * scale)),
    Math.max(1, Math.round(img.naturalHeight * scale)),
  );
  const normal = generatePseudoNormal(img, NORMAL_MAX_DIM);

  if (mapCache.size >= MAP_CACHE_MAX) {
    const oldest = mapCache.keys().next().value;
    if (oldest) mapCache.delete(oldest);
  }
  const entry = { normal, depth };
  mapCache.set(src, entry);
  return entry;
}

let renderer: SceneLightingRenderer | null = null;
let background: HTMLImageElement | null = null;
let observer: ResizeObserver | null = null;
let acquired = false;
let loadToken = 0;

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
  const scale = Math.max(width / background.naturalWidth, height / background.naturalHeight);
  const bgWidth = background.naturalWidth * scale;
  const bgHeight = background.naturalHeight * scale;
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

function teardown(): void {
  // 顺序要紧：先把立绘还给 DOM，再拆渲染器 —— 否则中途失败时人物会消失
  takeover.release();
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
}

function loadBackground(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    // Tauri 的 asset 协议与页面不同源，不声明跨域会在 texImage2D 时抛 SecurityError
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`背景加载失败: ${src}`));
    img.src = src;
  });
}

async function applyBackground(src: string): Promise<void> {
  if (!renderer) return;
  if (!src) {
    // 让位给了 WebGL、却连背景图都没有 → GameBackground 已经隐藏，会露出一块空白
    console.info("[lighting] 没有背景图，回退到 CSS 光影");
    teardown();
    return;
  }
  const token = ++loadToken;
  try {
    const img = await loadBackground(src);
    // 期间又换了背景或实例已销毁，丢弃这次结果
    if (token !== loadToken || !renderer) return;
    background = img;
    renderer.setBackground(img);
    draw();
  } catch (err) {
    // 背景图拿不到（协议不支持 / 文件缺失）时让出背景层，宁可退回 CSS 路径也不要留一块黑板
    console.warn("[lighting] 背景加载失败，回退到 CSS 光影", src, err);
    teardown();
  }
}

async function start(): Promise<void> {
  const canvas = canvasRef.value;
  if (!canvas) return;
  if (!hasLights.value) return;
  if (!SceneLightingRenderer.isSupported()) return;

  // 先让画布可见，否则 display:none 下 clientWidth 为 0，尺寸会被量成 1x1
  enabled.value = true;
  await nextTick();
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

  try {
    renderer = new SceneLightingRenderer(canvas);
  } catch (err) {
    // WebGL 不可用时静默退回旧路径，不让整屏背景因为光影挂掉
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
  draw();
}

watch(backgroundSrc, (src) => void applyBackground(src));

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

// 灯位/强度变化时重绘；不需要 rAF，场景参数不是逐帧动画
watch(lighting, () => draw(), { deep: true });

// 接管到的立绘换了图（表情/换装）→ 灌纹理。缓存命中时是同步的
watch(takeover.image, (img) => {
  if (!renderer || !img) return;
  const src = takeover.source.value;
  try {
    const maps = mapsFor(src, img);
    renderer.setCharacter(img, maps);
  } catch (err) {
    // 生成失败就交给渲染器自己算 auto，再不行它会回退平法线
    console.warn("[lighting] 立绘贴图准备失败，交回渲染器处理:", err);
    renderer.setCharacter(img, { normal: "auto", depth: "auto" });
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

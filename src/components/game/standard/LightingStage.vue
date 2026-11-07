<template>
  <!--
    2.5D 光影舞台。

    这一版只接管「背景」：当场景带 `lighting.lights` 时，用 WebGL 逐像素重画背景，
    并让 GameBackground 让出它那一层。立绘仍由 GameRolesStage 正常渲染，叠在
    本画布之上 —— 角色合成需要立绘的每帧位图，Live2D 拿不到，所以留到后续 PR。

    没有灯光的场景完全不启用，旧路径（CSS 滤镜 + 径向渐变）照旧。
  -->
  <canvas v-show="enabled" ref="canvasRef" class="lighting-stage" aria-hidden="true"></canvas>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { resolveBackgroundSrc } from "@/utils/background-src";
import { BackgroundLightingRenderer } from "@/render/lighting";
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
const backgroundSrc = computed(() => resolveBackgroundSrc(uiStore.currentBackground));

let renderer: BackgroundLightingRenderer | null = null;
let background: HTMLImageElement | null = null;
let observer: ResizeObserver | null = null;
let acquired = false;
let loadToken = 0;

/** 按设备像素比取画布尺寸；容器尺寸变了才重建缓冲。 */
function syncCanvasSize(): { width: number; height: number } {
  const canvas = canvasRef.value;
  if (!canvas) return { width: 0, height: 0 };
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const width = Math.max(1, Math.round(canvas.clientWidth * dpr));
  const height = Math.max(1, Math.round(canvas.clientHeight * dpr));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  return { width, height };
}

function draw(): void {
  if (!renderer || !background || !canvasRef.value) return;
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
    stage: { width, height },
    backgroundRect: bgRect,
  });
}

function teardown(): void {
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
  if (!src || !renderer) return;
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
  if (!BackgroundLightingRenderer.isSupported()) return;

  // 先让画布可见，否则 display:none 下 clientWidth 为 0，尺寸会被量成 1x1
  enabled.value = true;
  await nextTick();
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

  try {
    renderer = new BackgroundLightingRenderer(canvas);
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
  draw();
}

watch(backgroundSrc, (src) => void applyBackground(src));

watch(
  hasLights,
  async (has) => {
    if (has) await start();
    else teardown();
  },
  { immediate: true },
);

// 灯位/强度变化时重绘；不需要 rAF，场景参数不是逐帧动画
watch(lighting, () => draw(), { deep: true });

onMounted(() => {
  if (hasLights.value) void start();
});

onBeforeUnmount(() => {
  loadToken += 1;
  teardown();
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

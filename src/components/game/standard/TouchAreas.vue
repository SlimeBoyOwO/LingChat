<template>
  <div ref="rootRef" class="pointer-events-none absolute inset-0 z-30 overflow-hidden">
    <!-- viewBox 取 1×1 且 preserveAspectRatio 为 none：SVG 自己的盒子已经被摆成立绘的
         绘制矩形，于是 user 坐标就是图片归一化坐标，取点只需按元素矩形做一次除法 -->
    <svg
      v-if="rect"
      ref="svgRef"
      class="pointer-events-auto absolute"
      :style="rectStyle"
      viewBox="0 0 1 1"
      preserveAspectRatio="none"
      @click="handleClick"
    >
      <template v-for="(polygon, index) in polygons" :key="index">
        <polygon
          v-if="isGlowing"
          :points="toPoints(polygon)"
          vector-effect="non-scaling-stroke"
          class="polygon-glow"
        />
        <polygon
          v-if="isGlowing"
          :points="toPoints(polygon)"
          vector-effect="non-scaling-stroke"
          class="polygon-flow"
        />
      </template>
    </svg>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

import { loadImageAspect } from "@/composables/role/useRoleAvatar";
import { useGameStore } from "@/stores/modules/game";
import type { GameRole } from "@/stores/modules/game/state";
import {
  fitFromObjectFit,
  hitRegion,
  touchRegionMessage,
  imageRectInBox,
  parseBodyPart,
  resolveCostumeKey,
  type CostumeRegions,
  type LegacyFrame,
  type TouchPolygon,
} from "./touch-regions";

const props = defineProps<{
  role: GameRole;
  /** 与 ImageAcrossFade 用的是同一个取值，叠层据此还原立绘的实际落位 */
  objectFit: string;
  /** 立绘 URL，用来取图片自然尺寸算宽高比 */
  src: string;
}>();

const gameStore = useGameStore();

/** 连点保护。真正的闸门是 currentStatus，它挡住连点堆 LLM 回合。 */
const CLICK_DEBOUNCE_MS = 300;

const rootRef = ref<HTMLDivElement | null>(null);
const svgRef = ref<SVGSVGElement | null>(null);
const boxAspect = ref(0);
const imageAspect = ref(0);

const isGlowing = ref(false);
let glowTimeout: ReturnType<typeof setTimeout> | null = null;
let lastClickAt = 0;
let observer: ResizeObserver | null = null;
let imageToken = 0;

const fit = computed(() => fitFromObjectFit(props.objectFit));

const rect = computed(() => {
  if (!boxAspect.value || !imageAspect.value) return null;
  return imageRectInBox(boxAspect.value, imageAspect.value, fit.value);
});

const rectStyle = computed(() => {
  const value = rect.value;
  if (!value) return undefined;
  return {
    left: `${value.x * 100}%`,
    top: `${value.y * 100}%`,
    width: `${value.width * 100}%`,
    height: `${value.height * 100}%`,
  };
});

/** 旧坐标要靠当次的盒子与图片比例才能换算到图片坐标；拿不到就不渲染，宁可不显示也不画错 */
const legacyFrame = computed<LegacyFrame | null>(() => {
  if (!boxAspect.value || !imageAspect.value) return null;
  return { boxAspect: boxAspect.value, imageAspect: imageAspect.value, fit: fit.value };
});

const regions = computed<CostumeRegions | null>(() => {
  const { costumes } = parseBodyPart(props.role.bodyPart, legacyFrame.value);
  return costumes[resolveCostumeKey(props.role.clothesName)] ?? null;
});

const polygons = computed<TouchPolygon[]>(() =>
  Object.values(regions.value ?? {}).flatMap((region) => region.polygons),
);

const toPoints = (polygon: TouchPolygon) => polygon.map(([x, y]) => `${x},${y}`).join(" ");

function measureBox() {
  const element = rootRef.value;
  if (!element) return;
  const { width, height } = element.getBoundingClientRect();
  boxAspect.value = height > 0 ? width / height : 0;
}

async function measureImage(url: string) {
  const token = ++imageToken;
  imageAspect.value = 0;
  // 取不到自然尺寸就不渲染区域，静默降级
  const aspect = await loadImageAspect(url);
  if (token !== imageToken || !aspect) return;
  imageAspect.value = aspect;
}

function handleClick(event: MouseEvent) {
  if (gameStore.currentStatus !== "input") return;
  const svgRect = svgRef.value?.getBoundingClientRect();
  if (!svgRect?.width || !svgRect.height || !regions.value) return;
  const part = hitRegion(
    regions.value,
    (event.clientX - svgRect.left) / svgRect.width,
    (event.clientY - svgRect.top) / svgRect.height,
  );
  if (!part) return;
  const now = Date.now();
  if (now - lastClickAt < CLICK_DEBOUNCE_MS) return;
  lastClickAt = now;

  const message = touchRegionMessage(regions.value[part]?.message, gameStore.userName);
  gameStore.currentStatus = "thinking";
  invoke("send_system_message", { text: message }).catch((error) => {
    console.error("发送消息失败:", error);
    gameStore.currentStatus = "input";
  });
}

watch(() => props.src, measureImage, { immediate: true });

watch(
  () => gameStore.command,
  (command) => {
    if (glowTimeout) {
      clearTimeout(glowTimeout);
      glowTimeout = null;
    }
    if (command !== "touch") {
      isGlowing.value = false;
      return;
    }
    // 进触摸模式时脉冲三秒，提示哪些地方能摸
    isGlowing.value = true;
    glowTimeout = setTimeout(() => {
      isGlowing.value = false;
      glowTimeout = null;
    }, 3000);
  },
  { immediate: true },
);

onMounted(() => {
  measureBox();
  observer = new ResizeObserver(measureBox);
  if (rootRef.value) observer.observe(rootRef.value);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
  if (glowTimeout) clearTimeout(glowTimeout);
  glowTimeout = null;
});
</script>

<style scoped>
@reference "tailwindcss";

/* 触摸区域高亮整体不用 filter。原实现用一条四层 drop-shadow 链并逐帧改它的
   半径，每帧都要重跑滤镜图；即使改成常量，单次栅格化这条链也仍有一次可见的
   顿挫。改为纯描边 + 虚线流动，只走绘制路径。
   颜色取青色（同 GameDialog 分割线与桌宠外框）：白色描边在浅色立绘上看不见。 */
.polygon-glow {
  fill: none;
  stroke: rgba(34, 211, 238, 0.4);
  stroke-width: 2;
  animation: border-glow-pulse 3s ease-in-out forwards;
}

/* 亮段沿轮廓流动：dasharray 定出"一段亮 + 一段空"，动画只推 dashoffset，
   一个周期内首尾相接，循环无缝。*/
.polygon-flow {
  fill: none;
  stroke: rgba(34, 211, 238, 0.95);
  stroke-width: 2;
  stroke-dasharray: 10 46;
  animation:
    border-glow-pulse 3s ease-in-out forwards,
    border-glow-flow 1.6s linear infinite;
}

@keyframes border-glow-pulse {
  0% {
    opacity: 0;
  }
  15% {
    opacity: 1;
  }
  50% {
    opacity: 1;
  }
  70% {
    opacity: 0.75;
  }
  85% {
    opacity: 0.35;
  }
  100% {
    opacity: 0;
  }
}

/* 位移量等于 dasharray 一个周期（10 + 46），结尾与开头重合，循环处不跳变 */
@keyframes border-glow-flow {
  from {
    stroke-dashoffset: 56;
  }
  to {
    stroke-dashoffset: 0;
  }
}
</style>

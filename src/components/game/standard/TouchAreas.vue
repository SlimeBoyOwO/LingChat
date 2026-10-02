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
          :points="toPoints(polygon)"
          vector-effect="non-scaling-stroke"
          class="polygon-shape"
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

  const message = regions.value[part]?.message || `${gameStore.userName}戳了一下你`;
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

.polygon-shape {
  fill: none;
  stroke: white;
  stroke-width: 0;
  transition: all 300ms ease-in-out;
}

.polygon-glow {
  fill: none;
  stroke: rgba(255, 255, 255, 0.9);
  stroke-width: 2;
  animation: border-glow-pulse 3s ease-in-out forwards;
  filter: drop-shadow(0 0 6px rgba(255, 255, 255, 0.9))
    drop-shadow(0 0 12px rgba(255, 255, 255, 0.7)) drop-shadow(0 0 20px rgba(255, 255, 255, 0.5))
    drop-shadow(0 0 30px rgba(100, 200, 255, 0.4));
}

@keyframes border-glow-pulse {
  0% {
    stroke: rgba(255, 255, 255, 0);
    stroke-width: 0;
    filter: drop-shadow(0 0 0 rgba(255, 255, 255, 0)) drop-shadow(0 0 0 rgba(255, 255, 255, 0))
      drop-shadow(0 0 0 rgba(255, 255, 255, 0)) drop-shadow(0 0 0 rgba(100, 200, 255, 0));
  }
  15% {
    stroke: rgba(255, 255, 255, 1);
    stroke-width: 2;
    filter: drop-shadow(0 0 8px rgba(255, 255, 255, 1))
      drop-shadow(0 0 16px rgba(255, 255, 255, 0.8)) drop-shadow(0 0 28px rgba(255, 255, 255, 0.6))
      drop-shadow(0 0 40px rgba(100, 200, 255, 0.5));
  }
  30% {
    stroke: rgba(255, 255, 255, 0.8);
    stroke-width: 1.5;
    filter: drop-shadow(0 0 5px rgba(255, 255, 255, 0.7))
      drop-shadow(0 0 10px rgba(255, 255, 255, 0.5)) drop-shadow(0 0 18px rgba(255, 255, 255, 0.4))
      drop-shadow(0 0 25px rgba(100, 200, 255, 0.3));
  }
  50% {
    stroke: rgba(255, 255, 255, 1);
    stroke-width: 2.5;
    filter: drop-shadow(0 0 10px rgba(255, 255, 255, 1))
      drop-shadow(0 0 20px rgba(255, 255, 255, 0.8)) drop-shadow(0 0 35px rgba(255, 255, 255, 0.6))
      drop-shadow(0 0 50px rgba(100, 200, 255, 0.5));
  }
  70% {
    stroke: rgba(255, 255, 255, 0.6);
    stroke-width: 1.5;
    filter: drop-shadow(0 0 6px rgba(255, 255, 255, 0.6))
      drop-shadow(0 0 12px rgba(255, 255, 255, 0.4)) drop-shadow(0 0 20px rgba(255, 255, 255, 0.3))
      drop-shadow(0 0 30px rgba(100, 200, 255, 0.25));
  }
  85% {
    stroke: rgba(255, 255, 255, 0.3);
    stroke-width: 1;
    filter: drop-shadow(0 0 3px rgba(255, 255, 255, 0.4))
      drop-shadow(0 0 6px rgba(255, 255, 255, 0.2)) drop-shadow(0 0 10px rgba(255, 255, 255, 0.15))
      drop-shadow(0 0 15px rgba(100, 200, 255, 0.1));
  }
  100% {
    stroke: rgba(255, 255, 255, 0);
    stroke-width: 0;
    filter: drop-shadow(0 0 0 rgba(255, 255, 255, 0)) drop-shadow(0 0 0 rgba(255, 255, 255, 0))
      drop-shadow(0 0 0 rgba(255, 255, 255, 0)) drop-shadow(0 0 0 rgba(100, 200, 255, 0));
  }
}
</style>

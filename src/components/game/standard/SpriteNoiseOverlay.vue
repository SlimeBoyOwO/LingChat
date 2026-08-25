<template>
  <!-- 立绘噪点侵蚀覆盖层（DDLC n_rects_ghost 同款）：每帧随机抖动的黑色矩形团，
       盖住立绘的眼/嘴。定位盒用与立绘图片相同的宽高比+底对齐，使百分比坐标
       始终相对立绘本体而非外层容器。 -->
  <div ref="rootRef" class="sprite-noise-overlay" :style="overlayStyle">
    <div class="noise-sprite-box">
      <div
        v-for="(cluster, ci) in clusters"
        :key="ci"
        :ref="(el) => setClusterRef(el, ci)"
        class="noise-cluster"
        :style="{
          left: cluster.x + '%',
          top: cluster.y + '%',
          width: cluster.w + '%',
          height: cluster.h + '%',
        }"
      >
        <div v-for="ri in RECT_COUNT" :key="ri" class="noise-rect" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import type { CSSProperties } from 'vue'

/**
 * 噪点团布局（相对立绘图片的百分比，以钦灵 1071×1600 立绘实测标定）。
 * x/y = 团左上角；w/h = 团尺寸。眼睛团略大于眼眶，让噪点"溢出来"更瘆人。
 */
const CLUSTER_LAYOUTS: Record<string, Array<{ x: number; y: number; w: number; h: number }>> = {
  eyes: [
    { x: 37.5, y: 19.4, w: 9.5, h: 3.6 }, // 左眼
    { x: 49.5, y: 19.4, w: 9.5, h: 3.6 }, // 右眼
  ],
  mouth: [{ x: 43.5, y: 24.6, w: 8.0, h: 2.8 }],
}
CLUSTER_LAYOUTS.eyes_mouth = [...CLUSTER_LAYOUTS.eyes, ...CLUSTER_LAYOUTS.mouth]

const RECT_COUNT = 5 // 每团黑色矩形数（DDLC 用 4 个，略加密度）
const TICK_MS = 1000 / 30 // 30fps 随机重排，够抖又省性能

const props = withDefaults(
  defineProps<{
    /** 预设：'eyes' / 'mouth' / 'eyes_mouth'（未知值按 eyes_mouth 处理） */
    noise: string
    /** 淡入秒数（0 = 立即全显） */
    fadeInSec?: number
  }>(),
  { fadeInSec: 0 },
)

const clusters = computed(() => CLUSTER_LAYOUTS[props.noise] ?? CLUSTER_LAYOUTS.eyes_mouth)

const rootRef = ref<HTMLElement | null>(null)
const clusterEls: Array<HTMLElement | null> = []
const setClusterRef = (el: unknown, ci: number) => {
  clusterEls[ci] = (el as HTMLElement) ?? null
}

// 淡入：先按 fadeInSec 设 transition 并把 opacity 钉在 0，挂载后一帧再放到 1
const shown = ref(false)
const overlayStyle = computed<CSSProperties>(() => ({
  transitionProperty: 'opacity',
  transitionDuration: `${Math.max(0, props.fadeInSec)}s`,
  transitionTimingFunction: 'ease-out',
  opacity: shown.value ? 1 : 0,
}))

let timerId = 0

/** DDLC RectCluster 同款：每隔一拍把团内所有矩形的位置/尺寸全部重新随机 */
function tick() {
  for (const clusterEl of clusterEls) {
    if (!clusterEl) continue
    const rects = clusterEl.children
    for (let i = 0; i < rects.length; i++) {
      const el = rects[i] as HTMLElement
      // 位置可在团外溢出 ±25%，矩形的宽高在团尺寸的 15%~70% 间抖动
      el.style.left = `${(Math.random() - 0.5) * 125}%`
      el.style.top = `${(Math.random() - 0.5) * 125}%`
      el.style.width = `${15 + Math.random() * 55}%`
      el.style.height = `${15 + Math.random() * 55}%`
    }
  }
}

onMounted(() => {
  // 下一帧再淡入，保证初始 opacity:0 先被渲染出来
  requestAnimationFrame(() => {
    shown.value = true
  })
  tick()
  timerId = window.setInterval(tick, TICK_MS)
})

onBeforeUnmount(() => {
  window.clearInterval(timerId)
})
</script>

<style scoped>
.sprite-noise-overlay {
  position: absolute;
  inset: 0;
  z-index: 3; /* 高于立绘与 flash 覆盖层，低于气泡/对话 UI */
  pointer-events: none;
}

/* 与立绘图片显示盒对齐：高 102%（同立绘 img 的 h-[102%]）、宽高比 = 立绘原图比、
   水平居中、底部对齐。窄屏 object-fit 裁剪模式下会有偏差，可接受。 */
.noise-sprite-box {
  position: absolute;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
  height: 102%;
  aspect-ratio: 1071 / 1600;
}

.noise-cluster {
  position: absolute;
  overflow: visible;
}

.noise-rect {
  position: absolute;
  background: #000;
  /* 轻微的血色底色让黑块不那么"干净"，贴近 DDLC 黑眼眶下缘的血色 */
  box-shadow: 0 1px 2px rgba(120, 0, 0, 0.55);
}
</style>

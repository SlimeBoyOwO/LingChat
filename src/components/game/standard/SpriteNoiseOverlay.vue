<template>
  <!-- 立绘噪点侵蚀覆盖层（DDLC n_rects_ghost 同款）：每帧随机抖动的黑色矩形团，
       盖住立绘的眼/嘴。定位盒用与立绘图片相同的宽高比+底对齐，使百分比坐标
       始终相对立绘本体而非外层容器。 -->
  <div ref="rootRef" class="sprite-noise-overlay" :style="overlayStyle">
    <div class="noise-sprite-box" :style="spriteBoxStyle">
      <div
        v-for="(cluster, ci) in clusters"
        :key="ci"
        :ref="(el) => setClusterRef(el, ci)"
        :class="['noise-cluster', `noise-cluster--${cluster.kind}`]"
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
 * 噪点团布局（相对立绘图片的百分比，以钦灵 1071×1600 立绘逐像素标定）。
 * x/y = 团左上角；w/h = 团尺寸。矩形会围绕团中心轻微溢出，不能再把
 * x/y 当成抖动中心，否则整团会系统性向左上偏移。
 */
type NoiseCluster = {
  x: number
  y: number
  w: number
  h: number
  kind: 'eye' | 'mouth'
}

const CLUSTER_LAYOUTS: Record<string, NoiseCluster[]> = {
  eyes: [
    { x: 40.8, y: 20.8, w: 7.2, h: 3.8, kind: 'eye' }, // 左眼
    { x: 50.4, y: 20.8, w: 8.0, h: 3.8, kind: 'eye' }, // 右眼
  ],
  mouth: [{ x: 47.2, y: 24.2, w: 5.8, h: 1.9, kind: 'mouth' }],
}
CLUSTER_LAYOUTS.eyes_mouth = [...CLUSTER_LAYOUTS.eyes, ...CLUSTER_LAYOUTS.mouth]

const RECT_COUNT = 5 // 每团黑色矩形数（DDLC 用 4 个，略加密度）
const RECT_OVERFLOW = 20 // 相对团边界最多溢出 20%，但随机分布始终以团中心为中心
const TICK_MS = 1000 / 30 // 30fps 随机重排，够抖又省性能

const props = withDefaults(
  defineProps<{
    /** 预设：'eyes' / 'mouth' / 'eyes_mouth'（未知值按 eyes_mouth 处理） */
    noise: string
    /** 淡入秒数（0 = 立即全显） */
    fadeInSec?: number
    /** 与静态立绘共用的 CSS background-size（contain / auto N%） */
    objectFit?: string
  }>(),
  { fadeInSec: 0, objectFit: 'contain' },
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

/**
 * ImageAcrossFade 把立绘画在 102% 高的盒内；窄屏时 background-size 会变成
 * `auto N%`。噪点盒必须使用同一 N 并保持底对齐，否则眼睛会随窄屏缩放向上漂。
 */
const spriteBoxStyle = computed<CSSProperties>(() => {
  const match = /^auto\s+([\d.]+)%$/i.exec(props.objectFit.trim())
  const imagePercent = match ? Math.min(100, Math.max(0, Number(match[1]))) : 100
  return { height: `${1.02 * imagePercent}%` }
})

let timerId = 0

/** DDLC RectCluster 同款：每隔一拍把团内所有矩形的位置/尺寸全部重新随机 */
function tick() {
  for (const clusterEl of clusterEls) {
    if (!clusterEl) continue
    const rects = clusterEl.children
    for (let i = 0; i < rects.length; i++) {
      const el = rects[i] as HTMLElement
      // 先确定尺寸，再在含溢出边界的区域内摆放；公式保证随机矩形中心的
      // 期望值始终是团中心 50%，不会像旧公式那样围绕左上角 (0,0) 抖动。
      const width = 15 + Math.random() * 55
      const height = 15 + Math.random() * 55
      const left = -RECT_OVERFLOW + Math.random() * (100 + RECT_OVERFLOW * 2 - width)
      const top = -RECT_OVERFLOW + Math.random() * (100 + RECT_OVERFLOW * 2 - height)
      el.style.left = `${left}%`
      el.style.top = `${top}%`
      el.style.width = `${width}%`
      el.style.height = `${height}%`
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

/* 与立绘图片显示盒对齐：高度由同一个 background-size 动态计算，宽高比取
   钦灵原图 1071×1600，并始终水平居中、底部对齐。 */
.noise-sprite-box {
  position: absolute;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
  aspect-ratio: 1071 / 1600;
}

.noise-cluster {
  position: absolute;
  overflow: visible;
}

/* 随机矩形负责抖动质感，但不能靠随机帧决定眼睛是否被盖住；双眼先铺一层
   稍微外扩的纯黑眼眶，保证任何一帧都完全遮住虹膜，再让噪点块在上面跳动。 */
.noise-cluster--eye::before {
  content: '';
  position: absolute;
  inset: -8% -5%;
  background: #000;
  border-radius: 38% 44% 42% 36%;
  box-shadow:
    0 1px 2px rgba(120, 0, 0, 0.72),
    3px 0 0 rgba(0, 0, 0, 0.92),
    -2px 1px 0 rgba(0, 0, 0, 0.88);
}

.noise-rect {
  position: absolute;
  background: #000;
  /* 轻微的血色底色让黑块不那么"干净"，贴近 DDLC 黑眼眶下缘的血色 */
  box-shadow: 0 1px 2px rgba(120, 0, 0, 0.55);
}
</style>

<template>
  <div :style="{ opacity: containerOpacity }" class="touch-areas-container">
    <!-- 凸多边形区域 -->
    <svg
      class="polygon-area"
      :viewBox="`0 0 ${windowWidth} ${windowHeight}`"
      @click="handlePolygonClick"
    >
      <polygon
        :points="polygonPoints"
        fill="none"
        stroke="white"
        stroke-width="3"
        stroke-dasharray="8,4"
        class="polygon-shape"
      />
    </svg>

    <!-- 触摸音效播放器 -->
    <audio ref="touchAudio" preload="auto"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { scriptHandler } from '@/api/websocket/handlers/script-handler'
import { eventQueue } from '@/core/events/event-queue'

interface BodyPart {
  X: number[]
  Y: number[]
  windowWidth: number
  windowHeight: number
  message: string
}

interface Props {
  gameStore: any
  part?: BodyPart
  partKey?: string
}

const props = withDefaults(defineProps<Props>(), {
  part: () => ({
    X: [],
    Y: [],
    windowWidth: 1920,
    windowHeight: 1200,
    message: '',
  }),
  partKey: '',
})

const gameStore = useGameStore()
const uiStore = useUIStore()
const emit = defineEmits(['player-continued', 'dialog-proceed'])

const sent = ref(false)
const lastClickTime = ref(0)
const debounceDelay = 300
const touchCount = ref(0)

// 窗口尺寸
const windowWidth = ref(window.innerWidth)
const windowHeight = ref(window.innerHeight)

// 计算实际坐标点
const polygonPoints = computed(() => {
  const scale = windowHeight.value / props.part.windowHeight
  const centerX = windowWidth.value / 2
  const centerY = windowHeight.value / 2
  const originalCenterX = props.part.windowWidth / 2
  const originalCenterY = props.part.windowHeight / 2
  const points = []
  for (let i = 0; i < props.part.X.length; i++) {
    const originalX = props.part.X[i]! * props.part.windowWidth
    const originalY = props.part.Y[i]! * props.part.windowHeight
    const dx = originalX - originalCenterX
    const dy = originalY - originalCenterY
    const scaledDx = dx * scale
    const scaledDy = dy * scale
    const x = centerX + scaledDx
    const y = centerY + scaledDy
    points.push(`${x},${y}`)
  }
  return points.join(' ')
})

// 计算容器透明度
const containerOpacity = computed(() => {
  if (props.gameStore.command === 'show') {
    return 1
  } else if (props.gameStore.command === 'unshow') {
    return 0.001
  }
  return 0.001 // 默认不可见
})

// 射线法判断点是否在多边形内
const isPointInPolygon = (x: number, y: number, polygon: readonly [number, number][]): boolean => {
  let inside = false
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
    if (
      (polygon[i]?.[1] ?? 0 > y) !== (polygon[j]?.[1] ?? 0 > y) &&
      x <
        (((polygon[j]?.[0] ?? 0) - (polygon[i]?.[0] ?? 0)) * (y - (polygon[i]?.[1] ?? 0))) /
          ((polygon[j]?.[1] ?? 0) - (polygon[i]?.[1] ?? 0)) +
          (polygon[i]?.[0] ?? 0)
    ) {
      inside = !inside
    }
  }
  return inside
}

// 处理多边形点击
const handlePolygonClick = (event: MouseEvent) => {
  // 防抖检查：如果距离上次点击时间不足 debounceDelay 毫秒，则忽略此次点击
  const currentTime = Date.now()
  if (currentTime - lastClickTime.value < debounceDelay) {
    return
  }
  lastClickTime.value = currentTime

  // 检查当前是否处于触摸模式
  if (gameStore.command === 'touch' && event.target && gameStore.currentStatus == 'input') {
    const rect = (event.target as SVGElement).getBoundingClientRect()
    const x = event.clientX - rect.left
    const y = event.clientY - rect.top

    // 构建多边形坐标数组
    const scale = windowHeight.value / props.part.windowHeight
    const centerX = windowWidth.value / 2
    const centerY = windowHeight.value / 2
    const originalCenterX = props.part.windowWidth / 2
    const originalCenterY = props.part.windowHeight / 2
    const polygon: [number, number][] = props.part.X.map((nx, i) => {
      const originalX = nx * props.part.windowWidth
      const originalY = props.part.Y[i]! * props.part.windowHeight
      const dx = originalX - originalCenterX
      const dy = originalY - originalCenterY
      const scaledDx = dx * scale
      const scaledDy = dy * scale
      const x = centerX + scaledDx
      const y = centerY + scaledDy
      return [x, y]
    })

    if (isPointInPolygon(event.clientX, event.clientY, polygon)) {
      if (!sent.value) {
        gameStore.currentStatus = 'thinking'
        touchCount.value++
        const messageWithCount =
          touchCount.value === 1
            ? props.part.message
            : `${props.part.message},这是第${touchCount.value}次`
        scriptHandler.sendMessage(messageWithCount)
        sent.value = true
      } else {
        const needWait = eventQueue.continue()
        if (!needWait) {
          emit('player-continued')
          emit('dialog-proceed')
        }
        sent.value = false
      }
    }
  }
}

// 监听窗口大小变化
const handleResize = () => {
  windowWidth.value = window.innerWidth
  windowHeight.value = window.innerHeight
}

onMounted(() => {
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
})
</script>

<style scoped>
@reference "tailwindcss";

.touch-areas-container {
  @apply fixed inset-0 pointer-events-none z-100;
}

.polygon-area {
  @apply w-full h-full pointer-events-auto;
}

.polygon-shape {
  @apply fill-white/10 stroke-white stroke-[3] [stroke-dasharray:8,4] transition-colors duration-300 ease-in-out hover:fill-white/20;
}
</style>

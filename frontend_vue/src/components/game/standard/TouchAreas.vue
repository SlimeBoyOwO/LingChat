<template>
  <div :style="{ opacity: containerOpacity }" class="touch-areas-container">
    <!-- 单个 SVG 包含所有多边形 -->
    <svg
      class="polygon-area"
      :viewBox="`0 0 ${windowWidth} ${windowHeight}`"
      :width="windowWidth"
      :height="windowHeight"
      @click="handlePolygonClick"
    >
      <!-- 遍历过滤后的 bodyPart，每个渲染两个 polygon（发光层 + 主层） -->
      <template v-for="(part, key) in filteredBodyParts" :key="key">
        <!-- 发光层 - 仅在 isGlowing 时显示 -->
        <polygon
          v-if="isGlowing"
          :points="getPolygonPoints(part)"
          fill="none"
          stroke="white"
          stroke-width="2"
          class="polygon-glow-effect"
        />
        <!-- 主 polygon - stroke-width 始终为 0 -->
        <polygon
          :points="getPolygonPoints(part)"
          fill="none"
          stroke="white"
          stroke-width="0"
          stroke-dasharray="8,4"
          class="polygon-shape"
        />
      </template>
    </svg>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { scriptHandler } from '@/api/websocket/handlers/script-handler'
import { eventQueue } from '@/core/events/event-queue'

interface BodyPart {
  X: number[]
  Y: number[]
  windowWidth: number
  windowHeight: number
  message: string
  clothesName?: string
}

interface BodyParts {
  [key: string]: BodyPart
}

interface Props {
  bodyParts?: BodyParts | Record<string, BodyPart> | object
}

const props = withDefaults(defineProps<Props>(), {
  bodyParts: () => ({}),
})

const gameStore = useGameStore()
const emit = defineEmits(['player-continued', 'dialog-proceed'])

const windowWidth = ref(window.innerWidth)
const windowHeight = ref(window.innerHeight)

// 过滤后的 bodyParts，使用 ref + watch 确保响应式更新
const filteredBodyParts = ref<Record<string, BodyPart>>({})

// 标记是否发过消息
const sent = ref(false)

// 防抖
const lastClickTime = ref(0)
const debounceDelay = 300

// 发光效果状态
const isGlowing = ref(false)
let glowTimeout: ReturnType<typeof setTimeout> | null = null

// 根据 clothesName 过滤 bodyParts
// key 格式为 "head_{clothesName}"，只显示匹配当前衣服的 bodyPart
const updateFilteredBodyParts = () => {
  const clothesName = gameStore.mainRole?.clothesName ?? ''
  const parts = props.bodyParts as Record<string, BodyPart>
  const filtered: Record<string, BodyPart> = {}

  for (const key in parts) {
    const clothes = parts[key]?.clothesName
    if (clothesName === '' || clothes === clothesName) {
      filtered[key] = parts[key]!
    }
  }

  filteredBodyParts.value = filtered
}

// 监听 clothesName 变化
watch(
  () => gameStore.mainRole?.clothesName,
  () => {
    updateFilteredBodyParts()
  },
  { immediate: true },
)

// 监听 command 变化，触发发光效果
watch(
  () => gameStore.command,
  (newCommand) => {
    if (newCommand === 'touch') {
      isGlowing.value = true
      if (glowTimeout) {
        clearTimeout(glowTimeout)
      }

      // 3秒后关闭发光效果
      glowTimeout = setTimeout(() => {
        isGlowing.value = false
      }, 3000)
    } else {
      isGlowing.value = false
      if (glowTimeout) {
        clearTimeout(glowTimeout)
        glowTimeout = null
      }
    }
  },
  { immediate: true },
)

// 计算单个 bodyPart 的多边形坐标点
const getPolygonPoints = (part: BodyPart): string => {
  // bodyPart 需跟随人物移动，PC端横向分辨率变化不会使人物缩放
  const scale = windowHeight.value / part.windowHeight
  const centerX = windowWidth.value / 2
  const centerY = windowHeight.value / 2
  const originalCenterX = part.windowWidth / 2
  const originalCenterY = part.windowHeight / 2
  const points = []
  for (let i = 0; i < part.X.length; i++) {
    const originalX = part.X[i]! * part.windowWidth
    const originalY = part.Y[i]! * part.windowHeight
    const dx = originalX - originalCenterX
    const dy = originalY - originalCenterY
    const scaledDx = dx * scale
    const scaledDy = dy * scale
    const x = centerX + scaledDx
    const y = centerY + scaledDy
    points.push(`${x},${y}`)
  }
  return points.join(' ')
}

// 计算容器透明度
const containerOpacity = computed(() => {
  if (gameStore.command === 'touch') {
    return 1
  } else {
    return 0
  }
})

// 射线法判断点是否在多边形内
const isPointInPolygon = (x: number, y: number, polygon: [number, number][]): boolean => {
  let inside = false

  for (let i = 0, l = polygon.length, j = l - 1; i < l; j = i, i++) {
    let sx = polygon[i]![0] as number
    let sy = polygon[i]![1] as number
    let tx = polygon[j]![0] as number
    let ty = polygon[j]![1] as number

    if ((sy < y && ty >= y) || (sy >= y && ty < y)) {
      let xx = sx + ((y - sy) * (tx - sx)) / (ty - sy)

      if (xx > x) {
        inside = !inside
      }
    }
  }
  return inside
}

// 查找点击位置对应的多边形
const findClickedPart = (clientX: number, clientY: number): BodyPart | null => {
  const parts = filteredBodyParts.value
  for (const key in parts) {
    const part = parts[key]
    if (!part || !part.X || part.X.length === 0) continue

    const scale = windowHeight.value / part.windowHeight
    const centerX = windowWidth.value / 2
    const centerY = windowHeight.value / 2
    const originalCenterX = part.windowWidth / 2
    const originalCenterY = part.windowHeight / 2

    const polygon: [number, number][] = part.X.map((nx, i) => {
      const originalX = nx * part.windowWidth
      const originalY = part.Y[i]! * part.windowHeight
      const dx = originalX - originalCenterX
      const dy = originalY - originalCenterY
      const scaledDx = dx * scale
      const scaledDy = dy * scale
      const x = centerX + scaledDx
      const y = centerY + scaledDy
      return [x, y]
    })

    if (isPointInPolygon(clientX, clientY, polygon)) {
      return part
    }
  }
  return null
}

// 处理多边形点击
const handlePolygonClick = (event: MouseEvent) => {
  if (gameStore.command !== 'touch') {
    return
  }

  // 防抖检查：如果距离上次点击时间不足 debounceDelay 毫秒，则忽略此次点击
  const currentTime = Date.now()
  if (currentTime - lastClickTime.value < debounceDelay) {
    return
  }
  lastClickTime.value = currentTime

  // 检查当前是否处于触摸模式
  if (gameStore.command === 'touch' && event.target && gameStore.currentStatus == 'input') {
    // 查找点击的多边形
    const clickedPart = findClickedPart(event.clientX, event.clientY)

    if (clickedPart) {
      if (!sent.value) {
        gameStore.currentStatus = 'thinking'

        let message = ''
        const defaultMessage = `${gameStore.userName}戳了一下你`

        if (clickedPart.message) {
          message = clickedPart.message
        } else {
          message = defaultMessage
        }

        scriptHandler.sendMessage(message)
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
  // 清理发光效果定时器
  if (glowTimeout) {
    clearTimeout(glowTimeout)
    glowTimeout = null
  }
})
</script>

<style scoped>
@reference "tailwindcss";

.touch-areas-container {
  @apply fixed inset-0 z-2 pointer-events-none;
}

.polygon-area {
  @apply pointer-events-auto;
}

.polygon-shape {
  @apply stroke-white transition-all duration-300 ease-in-out;
  stroke-width: 0;
  fill: none;
}

/* 发光效果层 */
.polygon-glow-effect {
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

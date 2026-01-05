<template>
  <div v-if="isVisible" class="touch-areas-container">
    <!-- 凸多边形区域 -->
    <!-- <svg
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
    </svg> -->
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useGameStore } from '@/stores/modules/game'
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
  isVisible?: boolean
  part?: BodyPart
  partKey?: string
}

const props = withDefaults(defineProps<Props>(), {
  isVisible: true,
  part: () => ({
    X: [],
    Y: [],
    windowWidth: 1920,
    windowHeight: 1200,
    message: ""
  }),
  partKey: ""
})

const gameStore = useGameStore()
const emit = defineEmits(['area-clicked', 'player-continued', 'dialog-proceed'])

console.log("bodypart:", gameStore.avatar.body_part)
const sent = ref(false)

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

// 射线法判断点是否在多边形内
const isPointInPolygon = (x: number, y: number, polygon: readonly [number, number][]): boolean => {
  let inside = false
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
    if (
      polygon && polygon[i] && polygon[j] &&
      polygon[i][1] > y !== polygon[j][1] > y &&
      x <
        ((polygon[j][0] - polygon[i][0]) * (y - polygon[i][1])) / (polygon[j][1] - polygon[i][1]) +
          polygon[i][0]
    ) {
      inside = !inside
    }
  }
  return inside
}

function continueDialog(isPlayerTrigger: boolean): boolean {
  const needWait = eventQueue.continue()
  return needWait
}

// 处理多边形点击
const handlePolygonClick = (event: MouseEvent) => {
  // 检查当前是否处于触摸模式
  if (gameStore.command === 'touch' && event.target) {
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
        // alert(`X = [${props.part.X.join(', ')}]\nY = [${props.part.Y.join(', ')}]`)
        scriptHandler.sendMessage(props.part.message)
        sent.value = true
      } else {
        const needWait = continueDialog(true)
        if (!needWait) {
          emit('player-continued')
          emit('dialog-proceed')
        }
      }
      emit('area-clicked', props.partKey)
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

<!-- <style scoped>
.touch-areas-container {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 100;
}

.polygon-area {
  width: 100%;
  height: 100%;
  pointer-events: auto;
}

.polygon-shape {
  fill: rgba(255, 255, 255, 0.1);
  stroke: white;
  stroke-width: 3;
  stroke-dasharray: 8, 4;
  transition: fill 0.3s ease;
}

.polygon-shape:hover {
  fill: rgba(255, 255, 255, 0.2);
}
</style> -->
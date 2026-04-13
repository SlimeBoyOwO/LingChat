<template>
  <div class="w-full h-full flex flex-col bg-gray-900/50 rounded-lg p-4">
    <!-- Header -->
    <div class="flex items-center justify-between mb-4 pb-3 border-b border-gray-700">
      <h3 class="text-lg font-bold text-white">冒险总览</h3>
      <span class="text-sm text-gray-400">{{ completedCount }} / {{ totalCount }}</span>
    </div>

    <!-- Loading State -->
    <div v-if="loading" class="flex flex-col items-center justify-center py-12 text-gray-400">
      <div class="w-8 h-8 border-4 border-purple-500 border-t-transparent rounded-full animate-spin mb-2"></div>
      <p>加载中...</p>
    </div>

    <!-- Empty State -->
    <div v-else-if="adventures.length === 0" class="flex flex-col items-center justify-center py-12 text-gray-400">
      <p>暂无羁绊冒险</p>
    </div>

    <!-- Adventure Graph -->
    <div v-else class="relative flex-1 flex flex-col overflow-hidden">
      <!-- Zoom Controls -->
      <div class="absolute top-2 right-2 z-20 flex items-center gap-2 bg-gray-800/80 rounded-lg p-2">
        <button @click="zoomIn"
          class="w-8 h-8 flex items-center justify-center rounded bg-gray-700 hover:bg-gray-600 text-white transition-colors"
          title="放大">
          <svg class="w-5 h-5" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
            <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z" />
          </svg>
        </button>
        <span class="text-sm text-white font-mono w-12 text-center">{{ Math.round(zoomLevel * 100) }}%</span>
        <button @click="zoomOut"
          class="w-8 h-8 flex items-center justify-center rounded bg-gray-700 hover:bg-gray-600 text-white transition-colors"
          title="缩小">
          <svg class="w-5 h-5" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
            <path d="M19 13H5v-2h14v2z" />
          </svg>
        </button>
        <button @click="resetZoom"
          class="w-8 h-8 flex items-center justify-center rounded bg-gray-700 hover:bg-gray-600 text-white transition-colors"
          title="重置">
          <svg class="w-5 h-5" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
            <path
              d="M12 5V1L7 6l5 5V7c3.31 0 6 2.69 6 6s-2.69 6-6 6-6-2.69-6-6H4c0 4.42 3.58 8 8 8s8-3.58 8-8-3.58-8-8-8z" />
          </svg>
        </button>
      </div>

      <!-- Graph Container -->
      <div ref="graphWrapper" class="flex-1 overflow-auto relative">
        <div class="relative origin-top-left transition-transform duration-200 ease-out" :style="graphContainerStyle">
          <!-- SVG Connection Layer -->
          <svg class="absolute top-0 left-0 pointer-events-none z-0" :width="graphWidth" :height="graphHeight">
            <defs>
              <marker id="arrow-completed" markerWidth="12" markerHeight="12" refX="10" refY="4" orient="auto"
                markerUnits="strokeWidth">
                <path d="M0,0 L0,8 L12,4 z" fill="#4ade80" />
              </marker>
              <marker id="arrow-unlocked" markerWidth="12" markerHeight="12" refX="10" refY="4" orient="auto"
                markerUnits="strokeWidth">
                <path d="M0,0 L0,8 L12,4 z" fill="#c084fc" />
              </marker>
              <marker id="arrow-locked" markerWidth="12" markerHeight="12" refX="10" refY="4" orient="auto"
                markerUnits="strokeWidth">
                <path d="M0,0 L0,8 L12,4 z" fill="#6b7280" />
              </marker>
            </defs>

            <path v-for="connection in connections" :key="`${connection.from}-${connection.to}`" :d="connection.path"
              :class="connection.lineClass" :marker-end="connection.markerEnd" />
          </svg>

          <!-- Nodes Layer (扁平化遍历，纯数学绝对定位) -->
          <div class="relative z-10" :style="{ width: `${graphWidth}px`, height: `${graphHeight}px` }">
            <div v-for="adventure in adventures" :key="adventure.adventure_folder"
              class="absolute flex items-center gap-2 p-2 rounded-xl cursor-pointer transition-all duration-200 border-2"
              :class="getNodeClass(adventure)" :style="getNodeStyle(adventure.adventure_folder)"
              @click="handleNodeClick(adventure)">
              <!-- Icon -->
              <div class="w-14 h-14 flex items-center justify-center rounded-full shrink-0"
                :class="getIconClass(adventure.status)">
                <Book></Book>
              </div>

              <!-- Content -->
              <div class="flex-1 min-w-0">
                <div class="text-base font-semibold text-white mb-1">{{ adventure.name }}</div>
                <div class="text-sm text-gray-400 line-clamp-2">{{ adventure.description }}</div>
                <div v-if="adventure.status === 'locked'" class="flex items-center gap-1 mt-2 text-xs text-gray-500">
                  <span>🔒</span>
                  <span>{{ getUnlockHint(adventure) }}</span>
                </div>
                <div v-else class="flex items-center gap-1 mt-2 text-xs text-gray-500">
                  <div v-if="adventure.recommand_start">
                    <span>💡推荐开始：</span>
                    <span>{{ adventure.recommand_start }}</span>
                  </div>
                </div>
              </div>

              <!-- Status Badge -->
              <div class="shrink-0">
                <span class="px-3 py-1.5 rounded-full text-xs font-medium" :class="getBadgeClass(adventure.status)">
                  {{ getStatusText(adventure.status) }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useAdventureStore } from '@/stores/modules/adventure'
import type { AdventureInfo } from '@/api/services/adventure'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { Book } from 'lucide-vue-next'

interface Props {
  characterFolder: string
}

interface Connection {
  from: string
  to: string
  path: string
  lineClass: string
  markerEnd: string
}

const gameStore = useGameStore()
const uiStore = useUIStore()

const props = defineProps<Props>()
const adventureStore = useAdventureStore()

const loading = computed(() => adventureStore.loading)
const adventures = computed(() => adventureStore.sortedAdventures)
const completedCount = computed(() => adventureStore.completedCount)
const totalCount = computed(() => adventures.value.length)

// 缩放相关
const zoomLevel = ref(1)
const minZoom = 0.5
const maxZoom = 2
const zoomStep = 0.1

const zoomIn = () => (zoomLevel.value = Math.min(maxZoom, zoomLevel.value + zoomStep))
const zoomOut = () => (zoomLevel.value = Math.max(minZoom, zoomLevel.value - zoomStep))
const resetZoom = () => (zoomLevel.value = 1)

const graphContainerStyle = computed(() => ({
  transform: `scale(${zoomLevel.value})`,
  width: `${graphWidth.value}px`,
  height: `${graphHeight.value}px`,
}))

// 布局常量
const COLUMN_WIDTH = 480
const NODE_WIDTH = 380
const NODE_HEIGHT = 180
const NODE_GAP = 24
const PADDING_X = 40
const PADDING_Y = 40

// 列分配算法
const adventureColumns = computed(() => {
  const columns: AdventureInfo[][] = []
  const assigned = new Map<string, number>()
  const sorted = [...adventures.value].sort((a, b) => a.order - b.order)

  for (const adv of sorted) {
    const prereq = adv.unlock_conditions?.find((c) => c.type === 'adventure_completed')

    let columnIndex = 0
    if (prereq?.adventure_folder) {
      const prereqColumn = assigned.get(prereq.adventure_folder)
      if (prereqColumn !== undefined) {
        columnIndex = prereqColumn + 1
      }
    }

    while (columns.length <= columnIndex) {
      columns.push([])
    }

    columns[columnIndex]!.push(adv)
    assigned.set(adv.adventure_folder, columnIndex)
  }
  return columns
})

const graphWidth = computed(() => {
  const columnCount = adventureColumns.value.length
  return Math.max(800, PADDING_X * 2 + columnCount * COLUMN_WIDTH + NODE_WIDTH - COLUMN_WIDTH)
})

const graphHeight = computed(() => {
  let maxNodesInColumn = 1
  for (const column of adventureColumns.value) {
    maxNodesInColumn = Math.max(maxNodesInColumn, column.length)
  }
  const maxColumnHeight = maxNodesInColumn * NODE_HEIGHT + (maxNodesInColumn - 1) * NODE_GAP
  return Math.max(500, PADDING_Y * 2 + maxColumnHeight)
})

const nodeLayouts = computed(() => {
  const layouts = new Map<string, { x: number; y: number; width: number; height: number }>()

  adventureColumns.value.forEach((column, colIndex) => {
    const nodeCount = column.length
    const totalColumnHeight = nodeCount * NODE_HEIGHT + (nodeCount - 1) * NODE_GAP
    const startY = (graphHeight.value - totalColumnHeight) / 2

    column.forEach((adv, rowIndex) => {
      layouts.set(adv.adventure_folder, {
        x: PADDING_X + colIndex * COLUMN_WIDTH,
        y: startY + rowIndex * (NODE_HEIGHT + NODE_GAP),
        width: NODE_WIDTH,
        height: NODE_HEIGHT,
      })
    })
  })

  return layouts
})

// 提供给模板的独立节点样式
const getNodeStyle = (folder: string) => {
  const layout = nodeLayouts.value.get(folder)
  if (!layout) return { display: 'none' }
  return {
    left: `${layout.x}px`,
    top: `${layout.y}px`,
    width: `${layout.width}px`,
    height: `${layout.height}px`,
  }
}

const connections = computed<Connection[]>(() => {
  const result: Connection[] = []

  adventures.value.forEach((toAdv) => {
    const prereq = toAdv.unlock_conditions?.find((c) => c.type === 'adventure_completed')

    if (prereq?.adventure_folder) {
      const fromAdv = adventures.value.find((a) => a.adventure_folder === prereq.adventure_folder)
      if (!fromAdv) return

      // 直接从字典中取位置，精准无误
      const fromPos = nodeLayouts.value.get(fromAdv.adventure_folder)
      const toPos = nodeLayouts.value.get(toAdv.adventure_folder)

      if (!fromPos || !toPos) return

      // 计算连接点
      const fromX = fromPos.x + fromPos.width
      const fromY = fromPos.y + fromPos.height / 2
      const toX = toPos.x
      const toY = toPos.y + toPos.height / 2

      // 贝塞尔曲线控制点（稍微增大 offset 让曲线更平滑）
      const controlOffset = Math.max(40, Math.abs(toX - fromX) * 0.4)
      const path = `M ${fromX} ${fromY} C ${fromX + controlOffset} ${fromY}, ${toX - controlOffset} ${toY}, ${toX - 4} ${toY}`

      let lineClass = 'stroke-gray-600 stroke-2 fill-none stroke-dasharray-5'
      let markerEnd = 'url(#arrow-locked)'

      if (toAdv.status === 'completed') {
        lineClass = 'stroke-green-400 stroke-[2.5] fill-none'
        markerEnd = 'url(#arrow-completed)'
      } else if (toAdv.status !== 'locked') {
        lineClass = 'stroke-purple-400 stroke-[2.5] fill-none'
        markerEnd = 'url(#arrow-unlocked)'
      }

      result.push({
        from: fromAdv.adventure_folder,
        to: toAdv.adventure_folder,
        path,
        lineClass,
        markerEnd,
      })
    }
  })

  return result
})

onMounted(async () => {
  try {
    await adventureStore.fetchCharacterAdventures(props.characterFolder)
  } catch (error) {
    console.error('[AdventurePanel] Failed to fetch adventures:', error)
  }
})

// CSS 和逻辑处理保持你的原始逻辑
const getNodeClass = (adventure: AdventureInfo) => {
  const baseClass = 'bg-gray-800/50 border-gray-700 hover:bg-gray-800/80'
  switch (adventure.status) {
    case 'completed':
      return `${baseClass} border-green-500/50 bg-green-900/20 hover:bg-green-900/30 hover:shadow-lg hover:shadow-green-500/20`
    case 'unlocked':
    case 'in_progress':
      return `${baseClass} border-purple-500/50 bg-purple-900/20 hover:bg-purple-900/30 hover:shadow-lg hover:shadow-purple-500/20`
    case 'locked':
      return 'bg-gray-800/30 border-gray-700/50 opacity-60 cursor-not-allowed'
    default:
      return baseClass
  }
}

const getIconClass = (status: string) => {
  switch (status) {
    case 'completed':
      return 'bg-green-500/20 text-green-400'
    case 'locked':
      return 'bg-gray-700/50 text-gray-500'
    default:
      return 'bg-purple-500/20 text-purple-400'
  }
}

const getBadgeClass = (status: string) => {
  switch (status) {
    case 'completed':
      return 'bg-green-500/20 text-green-400'
    case 'in_progress':
      return 'bg-blue-500/20 text-blue-400'
    case 'unlocked':
      return 'bg-purple-500/20 text-purple-400'
    default:
      return 'bg-gray-700/50 text-gray-500'
  }
}

const getStatusText = (status: string) => {
  switch (status) {
    case 'completed':
      return '已完成'
    case 'in_progress':
      return '进行中'
    case 'unlocked':
      return '可游玩'
    default:
      return '未解锁'
  }
}

const getUnlockHint = (adventure: AdventureInfo): string => {
  if (!adventure.unlock_conditions || adventure.unlock_conditions.length === 0) return '自动解锁'
  const hints: string[] = []
  for (const cond of adventure.unlock_conditions) {
    if (cond.type === 'chat_count') hints.push(`对话${cond.threshold}次`)
    else if (cond.type === 'time_range') hints.push(`${cond.start_hour}:00-${cond.end_hour}:00`)
    else if (cond.type === 'adventure_completed') hints.push('完成前置冒险')
    else if (cond.type === 'achievement_unlocked') hints.push('解锁特定成就')
  }
  return hints.join(' · ')
}

const handleNodeClick = async (adventure: AdventureInfo) => {
  if (adventure.status === 'locked' || adventure.status === 'completed') return
  if (adventure.status === 'unlocked') {
    try {
      uiStore.showSettings = false
      gameStore.enterStoryMode(adventure.adventure_folder)
      await adventureStore.startAdventure(adventure.adventure_folder)
    } catch (error) {
      console.error('启动冒险失败:', error)
    }
  }
}
</script>

<style scoped>
.stroke-dasharray-5 {
  stroke-dasharray: 5, 5;
}
</style>

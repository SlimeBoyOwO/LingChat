<template>
  <div class="adventure-panel">
    <div class="panel-header">
      <h3 class="panel-title">羁绊冒险</h3>
      <div class="progress-info">
        <span class="progress-text">{{ completedCount }} / {{ totalCount }}</span>
      </div>
    </div>

    <div v-if="loading" class="loading-state">
      <div class="spinner"></div>
      <p>加载中...</p>
    </div>

    <div v-else-if="adventures.length === 0" class="empty-state">
      <p>暂无羁绊冒险</p>
    </div>

    <div v-else class="adventure-container">
      <!-- SVG连线层 -->
      <svg class="connection-layer" :viewBox="`0 0 ${containerWidth} ${containerHeight}`">
        <defs>
          <!-- 定义箭头标记 -->
          <marker
            id="arrow-unlocked"
            markerWidth="10"
            markerHeight="10"
            refX="9"
            refY="3"
            orient="auto"
            markerUnits="strokeWidth"
          >
            <path d="M0,0 L0,6 L9,3 z" fill="#a78bfa" />
          </marker>
          <marker
            id="arrow-locked"
            markerWidth="10"
            markerHeight="10"
            refX="9"
            refY="3"
            orient="auto"
            markerUnits="strokeWidth"
          >
            <path d="M0,0 L0,6 L9,3 z" fill="#6b7280" />
          </marker>
        </defs>

        <!-- 绘制连线 -->
        <path
          v-for="connection in connections"
          :key="`${connection.from}-${connection.to}`"
          :d="connection.path"
          :class="connection.isUnlocked ? 'connection-unlocked' : 'connection-locked'"
          :marker-end="connection.isUnlocked ? 'url(#arrow-unlocked)' : 'url(#arrow-locked)'"
        />
      </svg>

      <!-- 冒险节点列表 -->
      <div class="adventure-list">
        <div
          v-for="(adventure, index) in adventures"
          :key="adventure.adventure_folder"
          :ref="(el) => setNodeRef(el, index)"
          class="adventure-node"
          :class="getNodeClass(adventure)"
          @click="handleNodeClick(adventure)"
        >
          <div class="node-icon">
            <svg
              v-if="adventure.status === 'completed'"
              xmlns="http://www.w3.org/2000/svg"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
            </svg>
            <svg
              v-else-if="adventure.status === 'locked'"
              xmlns="http://www.w3.org/2000/svg"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path
                d="M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zm-6 9c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm3.1-9H8.9V6c0-1.71 1.39-3.1 3.1-3.1 1.71 0 3.1 1.39 3.1 3.1v2z"
              />
            </svg>
            <svg v-else xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2L2 7l10 5 10-5-10-5z" />
              <path d="M2 17l10 5 10-5" />
              <path d="M2 12l10 5 10-5" />
            </svg>
          </div>

          <div class="node-content">
            <div class="node-title">{{ adventure.name }}</div>
            <div class="node-description">{{ adventure.description }}</div>
            <div v-if="adventure.status === 'locked'" class="unlock-hint">
              <span class="hint-icon">🔒</span>
              <span class="hint-text">{{ getUnlockHint(adventure) }}</span>
            </div>
          </div>

          <div class="node-status">
            <span v-if="adventure.status === 'completed'" class="status-badge completed"
              >已完成</span
            >
            <span v-else-if="adventure.status === 'in_progress'" class="status-badge in-progress"
              >进行中</span
            >
            <span v-else-if="adventure.status === 'unlocked'" class="status-badge unlocked"
              >可游玩</span
            >
            <span v-else class="status-badge locked">未解锁</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, nextTick } from 'vue'
import { useAdventureStore } from '@/stores/modules/adventure'
import type { AdventureInfo } from '@/api/services/adventure'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'

interface Props {
  characterFolder: string
}

interface Connection {
  from: string
  to: string
  path: string
  isUnlocked: boolean
}

const gameStore = useGameStore()
const uiStore = useUIStore()

const props = defineProps<Props>()
const adventureStore = useAdventureStore()

const loading = computed(() => adventureStore.loading)
const adventures = computed(() => adventureStore.sortedAdventures)
const completedCount = computed(() => adventureStore.completedCount)
const totalCount = computed(() => adventures.value.length)

// 节点引用和容器尺寸
const nodeRefs = ref<(HTMLElement | null)[]>([])
const containerWidth = ref(600)
const containerHeight = ref(400)

// 设置节点引用
const setNodeRef = (el: any, index: number) => {
  if (el) {
    nodeRefs.value[index] = el as HTMLElement
  }
}

// 计算连线
const connections = computed<Connection[]>(() => {
  const result: Connection[] = []

  adventures.value.forEach((adventure, toIndex) => {
    // 检查是否有前置冒险条件
    const prereqCondition = adventure.unlock_conditions?.find(
      (cond) => cond.type === 'adventure_completed',
    )

    if (prereqCondition && prereqCondition.adventure_folder) {
      // 找到前置冒险的索引
      const fromIndex = adventures.value.findIndex(
        (adv) => adv.adventure_folder === prereqCondition.adventure_folder,
      )

      if (fromIndex !== -1 && nodeRefs.value[fromIndex] && nodeRefs.value[toIndex]) {
        const fromNode = nodeRefs.value[fromIndex]!
        const toNode = nodeRefs.value[toIndex]!

        // 计算节点中心位置
        const fromRect = fromNode.getBoundingClientRect()
        const toRect = toNode.getBoundingClientRect()
        const containerRect = fromNode.parentElement?.getBoundingClientRect()

        if (containerRect) {
          const fromX = fromRect.left - containerRect.left + fromRect.width / 2
          const fromY = fromRect.top - containerRect.top + fromRect.height / 2
          const toX = toRect.left - containerRect.left + toRect.width / 2
          const toY = toRect.top - containerRect.top + toRect.height / 2

          // 创建贝塞尔曲线路径
          const controlPointOffset = Math.abs(toY - fromY) * 0.5
          const path = `M ${fromX} ${fromY} C ${fromX} ${fromY + controlPointOffset}, ${toX} ${toY - controlPointOffset}, ${toX} ${toY}`

          // 判断连线是否应该显示为已解锁（目标冒险已解锁）
          const isUnlocked = adventure.status !== 'locked'

          result.push({
            from: adventures.value[fromIndex]!.adventure_folder,
            to: adventure.adventure_folder,
            path,
            isUnlocked,
          })
        }
      }
    }
  })

  return result
})

onMounted(async () => {
  console.log('[AdventurePanel] Mounting with characterFolder:', props.characterFolder)
  try {
    await adventureStore.fetchCharacterAdventures(props.characterFolder)
    console.log('[AdventurePanel] Fetched adventures:', adventureStore.currentCharacterAdventures)

    // 等待DOM更新后计算连线
    await nextTick()
    updateContainerSize()
  } catch (error) {
    console.error('[AdventurePanel] Failed to fetch adventures:', error)
  }
})

// 更新容器尺寸
const updateContainerSize = () => {
  if (nodeRefs.value.length > 0 && nodeRefs.value[0]) {
    const container = nodeRefs.value[0].parentElement
    if (container) {
      containerWidth.value = container.clientWidth
      containerHeight.value = container.clientHeight
    }
  }
}

const getNodeClass = (adventure: AdventureInfo) => {
  return {
    'node-completed': adventure.status === 'completed',
    'node-unlocked': adventure.status === 'unlocked' || adventure.status === 'in_progress',
    'node-locked': adventure.status === 'locked',
  }
}

const getUnlockHint = (adventure: AdventureInfo): string => {
  if (!adventure.unlock_conditions || adventure.unlock_conditions.length === 0) {
    return '自动解锁'
  }

  const hints: string[] = []
  for (const cond of adventure.unlock_conditions) {
    if (cond.type === 'chat_count') {
      hints.push(`对话${cond.threshold}次`)
    } else if (cond.type === 'time_range') {
      hints.push(`${cond.start_hour}:00-${cond.end_hour}:00`)
    } else if (cond.type === 'adventure_completed') {
      hints.push('完成前置冒险')
    } else if (cond.type === 'achievement_unlocked') {
      hints.push('解锁特定成就')
    }
  }

  return hints.join(' · ')
}

const handleNodeClick = async (adventure: AdventureInfo) => {
  if (adventure.status === 'locked') {
    // 显示解锁条件提示
    return
  }

  if (adventure.status === 'unlocked' || adventure.status === 'completed') {
    try {
      uiStore.showSettings = false
      gameStore.enterStoryMode(adventure.adventure_folder)
      await adventureStore.startAdventure(adventure.adventure_folder)
      // 启动成功后，可以触发其他UI变化或导航
    } catch (error) {
      console.error('启动冒险失败:', error)
    }
  }
}
</script>

<style scoped>
@reference "tailwindcss";

.adventure-panel {
  @apply w-full h-full flex flex-col bg-gray-900/50 rounded-lg p-4;
}

.panel-header {
  @apply flex items-center justify-between mb-4 pb-3 border-b border-gray-700;
}

.panel-title {
  @apply text-lg font-bold text-white;
}

.progress-info {
  @apply text-sm text-gray-400;
}

.loading-state,
.empty-state {
  @apply flex flex-col items-center justify-center py-12 text-gray-400;
}

.spinner {
  @apply w-8 h-8 border-4 border-purple-500 border-t-transparent rounded-full animate-spin mb-2;
}

.adventure-container {
  @apply relative flex-1 overflow-hidden;
}

.connection-layer {
  @apply absolute inset-0 pointer-events-none z-0;
}

.connection-unlocked {
  @apply stroke-purple-400;
  stroke-width: 2;
  fill: none;
  stroke-dasharray: none;
}

.connection-locked {
  @apply stroke-gray-600;
  stroke-width: 2;
  fill: none;
  stroke-dasharray: 5, 5;
}

.adventure-list {
  @apply relative flex flex-col gap-3 overflow-y-auto z-10;
}

.adventure-node {
  @apply flex items-center gap-3 p-3 rounded-lg cursor-pointer transition-all;
  @apply bg-gray-800/50 border border-gray-700;
}

.adventure-node:hover {
  @apply bg-gray-800/80;
}

.node-completed {
  @apply border-green-500/30 bg-green-900/10;
}

.node-completed:hover {
  @apply bg-green-900/20 shadow-lg shadow-green-500/20;
}

.node-unlocked {
  @apply border-purple-500/30 bg-purple-900/10;
}

.node-unlocked:hover {
  @apply bg-purple-900/20 shadow-lg shadow-purple-500/20;
}

.node-locked {
  @apply opacity-60 cursor-not-allowed;
}

.node-locked:hover {
  @apply bg-gray-800/50;
}

.node-icon {
  @apply w-10 h-10 flex items-center justify-center rounded-full shrink-0;
}

.node-completed .node-icon {
  @apply bg-green-500/20 text-green-400;
}

.node-unlocked .node-icon {
  @apply bg-purple-500/20 text-purple-400;
}

.node-locked .node-icon {
  @apply bg-gray-700/50 text-gray-500;
}

.node-icon svg {
  @apply w-6 h-6;
}

.node-content {
  @apply flex-1 min-w-0;
}

.node-title {
  @apply text-sm font-semibold text-white mb-1;
}

.node-description {
  @apply text-xs text-gray-400 line-clamp-2;
}

.unlock-hint {
  @apply flex items-center gap-1 mt-1 text-xs text-gray-500;
}

.node-status {
  @apply shrink-0;
}

.status-badge {
  @apply px-2 py-1 rounded text-xs font-medium;
}

.status-badge.completed {
  @apply bg-green-500/20 text-green-400;
}

.status-badge.in-progress {
  @apply bg-blue-500/20 text-blue-400;
}

.status-badge.unlocked {
  @apply bg-purple-500/20 text-purple-400;
}

.status-badge.locked {
  @apply bg-gray-700/50 text-gray-500;
}
</style>

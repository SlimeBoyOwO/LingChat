<template>
  <section
    class="code-assistant-panel"
    :class="{ 'is-collapsed': isCollapsed }"
    aria-label="Code 模式"
  >
    <!-- Header -->
    <header class="code-assistant-header">
      <div v-if="!isCollapsed" class="code-assistant-title">
        <span>Code 模式</span>
        <span class="code-beta">Beta</span>
      </div>
      <button
        class="icon-button collapse-button"
        type="button"
        :title="isCollapsed ? '展开 Code 模式' : '收起侧边栏'"
        @click="toggleCollapsed"
      >
        <ChevronsLeft v-if="isCollapsed" :size="18" />
        <ChevronsRight v-else :size="18" />
      </button>
    </header>

    <!-- 消息列表 -->
    <CodeMessageList
      :messages="displayMessages"
      :tool-logs="recentToolLogs"
      :show-status-bubble="showToolBubble"
      :status-bubble-text="toolBubbleText"
      :tool-preview="latestToolPreview"
      :is-thinking="gameStore.currentStatus === 'thinking'"
    />

    <!-- 输入区域 -->
    <CodeComposer
      ref="composerRef"
      :model-label="currentModelLabel"
      :status-text="statusText"
      :can-send="canSend"
      :is-touch-mode="gameStore.command === 'touch'"
      placeholder-text="Code 模式：输入代码任务，AI 会优先连续调用工具..."
      @send="handleSend"
      @open-settings="openTextSettings"
      @toggle-touch="touchMode.toggleTouchMode"
    />
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch, onBeforeUnmount, nextTick } from 'vue'
import { ChevronsLeft, ChevronsRight } from 'lucide-vue-next'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { scriptHandler } from '../../../api/websocket/handlers/script-handler'
import { eventQueue } from '../../../core/events/event-queue'
import CodeMessageList from './code/CodeMessageList.vue'
import CodeComposer from './code/CodeComposer.vue'
import { useTouchMode } from './code/useTouchMode'
import { useModelInfo } from './code/useModelInfo'

const gameStore = useGameStore()
const uiStore = useUIStore()

// ── 子组件 ──
const composerRef = ref<InstanceType<typeof CodeComposer> | null>(null)

// ── 面板折叠 ──
const isCollapsed = ref(false)

const toggleCollapsed = () => {
  isCollapsed.value = !isCollapsed.value
}

// ── Touch 模式（从 composable 导入） ──
const touchMode = useTouchMode()

// ── 模型信息（从 composable 导入） ──
const { currentModelLabel } = useModelInfo()

// ── 消息 / 日志计算 ──

const displayMessages = computed(() => (gameStore.dialogHistory || []).slice(-40))

const latestTool = computed(() => (uiStore.toolCallLogs || [])[0])

const autoHiddenToolIds = ref<Set<string>>(new Set())
const autoHideTimers = ref<Map<string, number>>(new Map())

watch(
  () => uiStore.toolCallLogs,
  (logs) => {
    for (const tool of logs) {
      const toolId = tool.id
      if (!toolId) continue
      if (tool.status === 'running') {
        if (autoHideTimers.value.has(toolId)) {
          clearTimeout(autoHideTimers.value.get(toolId)!)
          autoHideTimers.value.delete(toolId)
        }
        autoHiddenToolIds.value.delete(toolId)
        continue
      }
      if (!autoHideTimers.value.has(toolId) && !autoHiddenToolIds.value.has(toolId)) {
        const timer = window.setTimeout(() => {
          autoHiddenToolIds.value.add(toolId)
          autoHideTimers.value.delete(toolId)
        }, 5000)
        autoHideTimers.value.set(toolId, timer)
      }
    }
  },
  { deep: true, immediate: true },
)

const recentToolLogs = computed(() =>
  (uiStore.toolCallLogs || [])
    .filter((t) => t.id && !autoHiddenToolIds.value.has(t.id))
    .slice(0, 8)
    .slice()
    .reverse(),
)

const showToolBubble = computed(
  () =>
    (gameStore.currentStatus === 'thinking' || !!uiStore.activeToolStatusText) &&
    recentToolLogs.value.length === 0,
)

const toolBubbleText = computed(() => {
  if (uiStore.activeToolStatusText) return uiStore.activeToolStatusText
  if (gameStore.currentStatus === 'thinking') return '正在思考和执行...'
  return '工具执行完成'
})

const latestToolPreview = computed(() => latestTool.value?.preview || '')
const canSend = computed(() => gameStore.currentStatus === 'input')
const statusText = computed(() => (gameStore.currentStatus === 'thinking' ? '思考中' : '就绪'))

// ── 发送消息 ──

const handleSend = (text: string) => {
  if (gameStore.currentStatus === 'responding') {
    eventQueue.continue()
    return
  }

  gameStore.appendGameMessage({
    type: 'message',
    displayName: gameStore.userName || 'You',
    content: text,
  })

  gameStore.currentStatus = 'thinking'
  scriptHandler.sendMessage(text)
  nextTick(() => composerRef.value?.inputRef?.focus())
}

// ── 设置 ──

const openTextSettings = () => {
  uiStore.toggleSettings(true)
  uiStore.setSettingsTab('text')
}

// ── 清理 ──

onBeforeUnmount(() => {
  touchMode.cleanup()
  for (const timer of autoHideTimers.value.values()) {
    clearTimeout(timer)
  }
  autoHideTimers.value.clear()
})
</script>

<style scoped>
.code-assistant-panel {
  position: fixed;
  top: 4.25rem;
  right: 1.5rem;
  bottom: 1rem;
  z-index: 1;
  display: flex;
  width: min(27.5rem, calc(100vw - 3rem));
  min-height: 34rem;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid rgba(125, 211, 252, 0.3);
  border-radius: 22px;
  background:
    linear-gradient(145deg, rgba(15, 31, 57, 0.78), rgba(10, 24, 46, 0.88)),
    rgba(255, 255, 255, 0.08);
  box-shadow:
    0 22px 75px rgba(0, 0, 0, 0.42),
    inset 0 1px 1px rgba(255, 255, 255, 0.14);
  color: white;
  text-shadow: none;
  backdrop-filter: blur(32px) saturate(180%);
  -webkit-backdrop-filter: blur(32px) saturate(180%);
  transition:
    transform 720ms cubic-bezier(0.22, 0.8, 0.22, 1),
    opacity 720ms ease,
    box-shadow 720ms ease;
}

.code-assistant-panel.is-collapsed {
  opacity: 0.16;
  transform: translateX(calc(100% - 1.65rem));
  box-shadow: none;
}

.code-assistant-panel.is-collapsed:hover {
  opacity: 0.68;
  transform: translateX(calc(100% - 3.1rem));
}

.code-assistant-header {
  display: flex;
  min-height: 3.5rem;
  align-items: center;
  justify-content: space-between;
  padding: 0 1rem;
  border-bottom: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(255, 255, 255, 0.055);
}

.code-assistant-panel.is-collapsed .code-assistant-header {
  justify-content: flex-start;
  padding: 0 0.35rem;
  border-bottom: 0;
}

.code-assistant-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  color: rgb(248, 250, 252);
  font-size: 1rem;
  font-weight: 800;
}

.code-beta {
  border: 1px solid rgba(125, 211, 252, 0.4);
  border-radius: 999px;
  background: rgba(14, 165, 233, 0.18);
  padding: 0.1rem 0.45rem;
  color: rgb(186, 230, 253);
  font-size: 0.6875rem;
}

.icon-button {
  display: grid;
  width: 2rem;
  height: 2rem;
  place-items: center;
  border-radius: 10px;
  color: rgba(224, 242, 254, 0.75);
  background: none;
  border: none;
  cursor: pointer;
}

.collapse-button {
  position: absolute;
  top: 0.75rem;
  right: 0.875rem;
  border: 1px solid rgba(125, 211, 252, 0.22);
  background: rgba(255, 255, 255, 0.08);
  z-index: 2;
}

.code-assistant-panel.is-collapsed .collapse-button {
  left: 0.35rem;
  right: auto;
}

.icon-button:hover {
  background: rgba(255, 255, 255, 0.12);
  color: white;
}

@media (max-width: 720px) {
  .code-assistant-panel {
    left: 0.75rem;
    right: 0.75rem;
    top: 5.25rem;
    bottom: 0.75rem;
    width: auto;
    min-height: 0;
  }
}
</style>

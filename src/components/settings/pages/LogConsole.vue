<template>
  <!-- 日志已弹出到独立窗口：显示占位提示 -->
  <div v-if="!standalone && popped" class="popped-placeholder">
    <PictureInPicture2 :size="36" />
    <div class="popped-text">日志已弹出到独立窗口，请在独立窗口中查看</div>
    <button class="popback-btn" @click="popoutWindow">聚焦日志窗口</button>
  </div>

  <div v-else class="flex flex-col h-full min-h-0">
    <!-- Toolbar -->
    <div class="flex items-center justify-between mb-3 shrink-0 gap-3 flex-wrap">
      <div class="flex items-center gap-1.5">
        <button
          v-for="lvl in levels"
          :key="lvl.key"
          class="filter-btn"
          :class="{ active: isLevelVisible(lvl.key) }"
          :style="{
            '--lvl-color': lvl.color,
            '--lvl-bg': lvl.color + '22',
          }"
          @click="toggleLevel(lvl.key)"
        >
          {{ lvl.label }}
        </button>
      </div>

      <div class="flex items-center gap-2">
        <span class="text-sm text-gray-400">{{ visibleCount }} / {{ logs.length }}</span>

        <button
          v-if="!standalone"
          class="icon-btn"
          title="弹出独立窗口"
          @click="popoutWindow"
        >
          <PictureInPicture2 :size="14" />
        </button>

        <button
          class="icon-btn"
          :class="{ active: autoOpen }"
          title="启动时自动打开日志窗口"
          @click="toggleAutoOpen"
        >
          <Rocket :size="14" />
        </button>

        <button
          class="icon-btn"
          :class="{ active: autoScroll }"
          title="自动滚动到底部"
          @click="toggleAutoScroll"
        >
          <ArrowDown :size="14" />
        </button>

        <button
          class="icon-btn"
          :class="{ active: paused }"
          :title="paused ? '继续' : '暂停'"
          @click="paused = !paused"
        >
          <Pause v-if="!paused" :size="14" />
          <Play v-else :size="14" />
        </button>

        <button class="icon-btn" title="清空" @click="clearLogs">
          <Trash2 :size="14" />
        </button>
      </div>
    </div>

    <!-- Log area -->
    <div
      ref="logContainer"
      class="log-area scrollbar-thin flex-1 min-h-0 overflow-y-auto rounded-xl px-3 py-3"
      :class="{ 'log-area--standalone': standalone }"
      :style="{ scrollbarColor: 'var(--accent-color, #79d9ff) transparent' }"
      @scroll="handleScroll"
    >
      <div
        v-if="filteredLogs.length === 0"
        class="flex flex-1 items-center justify-center py-10"
      >
        <div class="text-center text-xl font-bold text-gray-100 opacity-60">暂无日志</div>
      </div>

      <template v-for="(entry, _idx) in filteredLogs" :key="_idx">
        <div
          :class="[
            'log-line',
            entry.level.toLowerCase(),
            { 'log-line--narrow': uiStore.isNarrowScreen },
          ]"
        >
          <span class="timestamp">{{ entry.timestamp }}</span>
          <span :class="['level-tag', entry.level.toLowerCase()]">{{ entry.level }}</span>
          <span class="target">{{ entry.target }}</span>
          <span class="message" :class="{ 'w-full': uiStore.isNarrowScreen }">{{ entry.message }}</span>
        </div>
      </template>

      <div
        v-if="paused && pendingCount > 0"
        class="mt-3 pt-3 border-t border-dashed border-yellow-500/30 text-center text-sm text-yellow-400"
      >
        已暂停 — {{ pendingCount }} 条新日志
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { useUIStore } from '@/stores/modules/ui/ui'
import { Pause, Play, Trash2, ArrowDown, PictureInPicture2, Rocket } from 'lucide-vue-next'

// standalone=true 时渲染在独立日志窗口里（隐藏“弹出独立窗口”按钮）
const props = withDefaults(defineProps<{ standalone?: boolean }>(), { standalone: false })

const uiStore = useUIStore()

interface LogEntry {
  timestamp: string
  level: string
  target: string
  message: string
}

const levels = [
  { key: 'ERROR', label: 'ERRO', color: '#f44747' },
  { key: 'WARN', label: 'WARN', color: '#e5c07b' },
  { key: 'INFO', label: 'INFO', color: '#98c379' },
  { key: 'DEBUG', label: 'DEBG', color: '#61afef' },
  { key: 'TRACE', label: 'TRCE', color: '#c678dd' },
]

const MAX_LOGS = 5000
const AUTO_OPEN_KEY = 'lingchat_log_window_auto_open'

const logs = ref<LogEntry[]>([])
const visibleLevels = ref(new Set<string>(levels.map((l) => l.key)))
const paused = ref(false)
const pendingCount = ref(0)
const autoScroll = ref(true)
const autoOpen = ref(localStorage.getItem(AUTO_OPEN_KEY) === '1')
const popped = ref(false)
const logContainer = ref<HTMLElement | null>(null)
let unlisten: UnlistenFn | null = null
let unlistenState: UnlistenFn | null = null

const filteredLogs = computed(() =>
  logs.value.filter((e) => visibleLevels.value.has(e.level.toUpperCase())),
)
const visibleCount = computed(() => filteredLogs.value.length)

function isLevelVisible(key: string) {
  return visibleLevels.value.has(key)
}

function toggleLevel(key: string) {
  const next = new Set(visibleLevels.value)
  if (next.has(key)) {
    next.delete(key)
  } else {
    next.add(key)
  }
  visibleLevels.value = next
}

function clearLogs() {
  logs.value = []
  pendingCount.value = 0
}

function scrollToBottom(force = false) {
  if (!force && !autoScroll.value) return
  nextTick(() => {
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight
    }
  })
}

function toggleAutoScroll() {
  autoScroll.value = !autoScroll.value
  if (autoScroll.value) {
    scrollToBottom(true)
  }
}

// 用户向上滚动时暂停自动滚动，回到底部时恢复
function handleScroll() {
  const el = logContainer.value
  if (!el) return
  const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40
  autoScroll.value = atBottom
}

// 弹出独立日志窗口（已存在则聚焦）
function popoutWindow() {
  invoke('open_log_window').catch((e) => console.error('[LogConsole] 打开日志窗口失败:', e))
}

// “启动时自动打开日志窗口”开关，持久化到 localStorage
function toggleAutoOpen() {
  autoOpen.value = !autoOpen.value
  localStorage.setItem(AUTO_OPEN_KEY, autoOpen.value ? '1' : '0')
}

onMounted(async () => {
  // 设置页场景：查询独立日志窗口状态，已弹出则显示占位提示
  if (!props.standalone) {
    try {
      popped.value = await invoke<boolean>('is_log_window_open')
    } catch (e) {
      console.warn('[LogConsole] Failed to query log window state:', e)
    }
    unlistenState = await listen<boolean>('log-window:state', (event) => {
      popped.value = event.payload
    })
  }

  // Fetch startup logs first
  try {
    const history = await invoke<LogEntry[]>('get_log_history')
    logs.value = history.slice(-MAX_LOGS)
    await nextTick()
    scrollToBottom(true)
  } catch (e) {
    console.warn('[LogConsole] Failed to fetch log history:', e)
  }

  // Then listen for live events
  unlisten = await listen<LogEntry>('log:entry', (event) => {
    if (paused.value) {
      pendingCount.value++
    } else {
      logs.value.push(event.payload)
      if (logs.value.length > MAX_LOGS) {
        logs.value = logs.value.slice(-MAX_LOGS)
      }
      scrollToBottom()
    }
  })
})

onUnmounted(() => {
  unlisten?.()
  unlistenState?.()
})

watch(paused, (now) => {
  if (!now && pendingCount.value > 0) {
    pendingCount.value = 0
    scrollToBottom()
  }
})
</script>

<style scoped>
/* 已弹出到独立窗口时的占位提示 */
.popped-placeholder {
  flex: 1;
  min-height: 0;
  max-height: 70vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
  border: 1px dashed rgba(255, 255, 255, 0.22);
  border-radius: 12px;
  background: rgba(0, 0, 0, 0.4);
  color: rgba(255, 255, 255, 0.55);
  padding: 32px 16px;
}
.popped-text {
  font-size: 14px;
  text-align: center;
  line-height: 1.8;
}
.popback-btn {
  padding: 6px 16px;
  border-radius: 8px;
  border: none;
  background: var(--accent-color, #79d9ff);
  color: #0b2530;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
}
.popback-btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 10px rgba(121, 217, 255, 0.4);
}

/* Filter level buttons — matching the project's button style */
.filter-btn {
  font-size: 11px;
  font-weight: 600;
  padding: 3px 10px;
  border-radius: 6px;
  border: 1px solid transparent;
  background: #e9ecef;
  color: #495057;
  cursor: pointer;
  transition: all 0.2s ease;
  letter-spacing: 0.3px;
}
.filter-btn:hover {
  background: var(--accent-color, #79d9ff);
  color: #fff;
  transform: translateY(-1px);
  box-shadow: 0 4px 10px rgba(121, 217, 255, 0.4);
}
.filter-btn.active {
  background: var(--lvl-bg);
  border-color: var(--lvl-color);
  color: var(--lvl-color);
}
.filter-btn.active:hover {
  background: var(--lvl-color);
  color: #fff;
}

/* Icon buttons */
.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  border: none;
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.6);
  cursor: pointer;
  transition: all 0.2s ease;
}
.icon-btn:hover {
  background: rgba(255, 255, 255, 0.15);
  color: #fff;
}
.icon-btn.active {
  background: rgba(121, 217, 255, 0.2);
  color: var(--accent-color, #79d9ff);
}

/* Log area — glass-morphism matching project style */
.log-area {
  background: rgba(0, 0, 0, 0.65);
  border: 1px solid rgba(255, 255, 255, 0.1);
  backdrop-filter: blur-md;
  font-family: 'Cascadia Code', 'Fira Code', 'JetBrains Mono', 'Consolas', monospace;
  font-size: 13px;
  line-height: 1.7;
  max-height: 70vh;
  overflow-x: hidden;
}

/* 独立窗口模式：日志区占满整个窗口高度 */
.log-area--standalone {
  max-height: none;
}

/* Log line — 终端式布局：元信息内联，长文本折行后占满整行宽度 */
.log-line {
  display: block;
  padding: 1px 0;
  border-radius: 2px;
}
.log-line:hover {
  background: rgba(255, 255, 255, 0.04);
}

/* Timestamp */
.timestamp {
  display: inline-block;
  margin-right: 10px;
  min-width: 88px;
  color: rgba(255, 255, 255, 0.28);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

/* Level badge */
.level-tag {
  display: inline-block;
  margin-right: 10px;
  width: 38px;
  font-size: 10px;
  font-weight: 700;
  text-align: center;
  border-radius: 3px;
  padding: 0 4px;
  line-height: 18px;
}
.level-tag.error {
  color: #f44747;
  background: rgba(244, 71, 71, 0.14);
}
.level-tag.warn {
  color: #e5c07b;
  background: rgba(229, 192, 123, 0.12);
}
.level-tag.info {
  color: #98c379;
  background: rgba(152, 195, 121, 0.1);
}
.level-tag.debug {
  color: #61afef;
  background: rgba(97, 175, 239, 0.12);
}
.level-tag.trace {
  color: #c678dd;
  background: rgba(198, 120, 221, 0.1);
}

/* Target module path */
.target {
  display: inline;
  margin-right: 10px;
  color: rgba(255, 255, 255, 0.4);
  font-size: 12px;
  overflow-wrap: anywhere;
}
.target::after {
  content: ':';
}

/* Message — 内联流式排列，折行后像终端一样占满整行 */
.message {
  display: inline;
  color: rgba(255, 255, 255, 0.85);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}

/* Per-level message color tint */
.log-line.error .message {
  color: #fca5a5;
}
.log-line.warn .message {
  color: #fde68a;
}
.log-line.trace .message {
  color: rgba(255, 255, 255, 0.45);
}

/* Narrow screen: 时间戳等元信息更紧凑 */
.log-line--narrow {
  padding: 2px 0;
}
.log-line--narrow .target::after {
  content: none;
}
</style>

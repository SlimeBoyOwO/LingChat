<template>
  <MenuPage>
    <MenuItem title="日志" size="large">
      <template #header>
        <ScrollText :size="20" />
      </template>

      <div class="flex flex-col h-full min-h-0">
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
          :style="{ scrollbarColor: 'var(--accent-color, #79d9ff) transparent' }"
        >
          <div
            v-if="filteredLogs.length === 0"
            class="flex flex-1 items-center justify-center py-10"
          >
            <div class="text-center text-xl font-bold text-gray-100 opacity-60">暂无日志</div>
          </div>

          <template v-for="(entry, _idx) in filteredLogs" :key="_idx">
            <div :class="['log-line', entry.level.toLowerCase()]">
              <span class="timestamp">{{ entry.timestamp }}</span>
              <span :class="['level-tag', entry.level.toLowerCase()]">{{ entry.level }}</span>
              <span class="target">{{ entry.target }}</span>
              <span class="message">{{ entry.message }}</span>
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
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { MenuPage, MenuItem } from '../../ui'
import { ScrollText, Pause, Play, Trash2 } from 'lucide-vue-next'

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

const logs = ref<LogEntry[]>([])
const visibleLevels = ref(new Set<string>(levels.map((l) => l.key)))
const paused = ref(false)
const pendingCount = ref(0)
const logContainer = ref<HTMLElement | null>(null)
let unlisten: UnlistenFn | null = null

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

function scrollToBottom() {
  nextTick(() => {
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight
    }
  })
}

onMounted(async () => {
  // Fetch startup logs first
  try {
    const history = await invoke<LogEntry[]>('get_log_history')
    logs.value = history.slice(-MAX_LOGS)
    await nextTick()
    scrollToBottom()
  } catch (e) {
    console.warn('[SettingsLog] Failed to fetch log history:', e)
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
})

watch(paused, (now) => {
  if (!now && pendingCount.value > 0) {
    pendingCount.value = 0
    scrollToBottom()
  }
})
</script>

<style scoped>
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
}

/* Log line */
.log-line {
  display: flex;
  gap: 10px;
  align-items: baseline;
  padding: 1px 0;
  border-radius: 2px;
}
.log-line:hover {
  background: rgba(255, 255, 255, 0.04);
}

/* Timestamp */
.timestamp {
  flex-shrink: 0;
  min-width: 88px;
  color: rgba(255, 255, 255, 0.28);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

/* Level badge */
.level-tag {
  flex-shrink: 0;
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
  flex-shrink: 0;
  color: rgba(255, 255, 255, 0.4);
  font-size: 12px;
}
.target::after {
  content: ':';
}

/* Message */
.message {
  color: rgba(255, 255, 255, 0.85);
  word-break: break-all;
  white-space: pre-wrap;
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
</style>

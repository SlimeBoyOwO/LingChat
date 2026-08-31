<template>
  <!-- 日志已弹出到独立窗口：显示占位提示 -->
  <div
    v-if="!standalone && popped"
    class="flex-1 min-h-0 max-h-[70vh] flex flex-col items-center justify-center gap-3.5 border border-dashed border-white/20 rounded-xl bg-black/40 text-white/55 px-4 py-8"
  >
    <PictureInPicture2 :size="36" />
    <div class="text-sm text-center leading-loose">{{ $t('settings.log.poppedOut') }}</div>
    <button
      class="px-4 py-1.5 rounded-lg border-none bg-(--accent-color,#79d9ff) text-[#0b2530] text-[13px] font-semibold cursor-pointer transition-all duration-200 hover:-translate-y-px hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)]"
      @click="popoutWindow"
    >
      {{ $t('settings.log.focusLog') }}
    </button>
  </div>

  <div v-else class="flex flex-col h-full min-h-0">
    <!-- Toolbar -->
    <div class="flex items-center justify-between mb-3 shrink-0 gap-3 flex-wrap">
      <div class="flex items-center gap-1.5">
        <button
          v-for="lvl in levels"
          :key="lvl.key"
          class="text-[11px] font-semibold px-2.5 py-[3px] rounded-md border border-transparent bg-[#e9ecef] text-[#495057] cursor-pointer transition-all duration-200 tracking-[0.3px] hover:bg-(--accent-color,#79d9ff) hover:text-white hover:-translate-y-px hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)]"
          :class="
            isLevelVisible(lvl.key)
              ? 'bg-(--lvl-bg)! border-(--lvl-color)! text-(--lvl-color)! hover:bg-(--lvl-color)! hover:text-white!'
              : ''
          "
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
          v-if="!standalone && canPopout"
          :class="ICON_BTN"
          :title="$t('settings.log.popout')"
          @click="popoutWindow"
        >
          <PictureInPicture2 :size="14" />
        </button>

        <button
          v-if="canPopout"
          :class="[ICON_BTN, autoOpen && ICON_BTN_ACTIVE]"
          :title="$t('settings.log.autoOpen')"
          @click="toggleAutoOpen"
        >
          <Rocket :size="14" />
        </button>

        <button
          :class="[ICON_BTN, autoScroll && ICON_BTN_ACTIVE]"
          :title="$t('settings.log.autoScroll')"
          @click="toggleAutoScroll"
        >
          <ArrowDown :size="14" />
        </button>

        <button
          :class="[ICON_BTN, paused && ICON_BTN_ACTIVE]"
          :title="paused ? $t('settings.log.resume') : $t('settings.log.pause')"
          @click="paused = !paused"
        >
          <Pause v-if="!paused" :size="14" />
          <Play v-else :size="14" />
        </button>

        <button :class="ICON_BTN" :title="$t('settings.log.clear')" @click="clearLogs">
          <Trash2 :size="14" />
        </button>
      </div>
    </div>

    <!-- Log area -->
    <div
      ref="logContainer"
      class="scrollbar-thin flex-1 min-h-0 overflow-y-auto overflow-x-hidden rounded-xl px-3 py-3 bg-black/65 border border-white/10 backdrop-blur-md text-[13px] leading-[1.7] font-['Cascadia_Code','Fira_Code','JetBrains_Mono','Consolas',monospace]"
      :class="standalone ? 'max-h-none' : 'max-h-[70vh]'"
      :style="{ scrollbarColor: 'var(--accent-color, #79d9ff) transparent' }"
      @scroll="handleScroll"
    >
      <div
        v-if="filteredLogs.length === 0"
        class="flex flex-1 items-center justify-center py-10"
      >
        <div class="text-center text-xl font-bold text-gray-100 opacity-60">{{ $t('settings.log.empty') }}</div>
      </div>

      <template v-for="(entry, _idx) in filteredLogs" :key="_idx">
        <!-- 终端式布局：元信息内联，长文本折行后占满整行宽度 -->
        <div
          class="block py-px rounded-sm hover:bg-white/[0.04]"
          :class="{ 'py-0.5': uiStore.isNarrowScreen }"
        >
          <span
            class="inline-block mr-2.5 min-w-[88px] text-white/30 text-xs tabular-nums"
            >{{ entry.timestamp }}</span
          >
          <span
            class="inline-block mr-2.5 w-[38px] text-[10px] font-bold text-center rounded-[3px] px-1 leading-[18px]"
            :class="levelTagClass(entry.level)"
            >{{ entry.level }}</span
          >
          <span
            class="inline mr-2.5 text-white/40 text-xs wrap-anywhere"
            :class="uiStore.isNarrowScreen ? 'after:content-none' : `after:content-[':']`"
            >{{ entry.target }}</span
          >
          <span
            class="inline text-white/85 whitespace-pre-wrap wrap-anywhere break-words"
            :class="[messageTintClass(entry.level), { 'w-full': uiStore.isNarrowScreen }]"
            >{{ entry.message }}</span
          >
        </div>
      </template>

      <div
        v-if="paused && pendingCount > 0"
        class="mt-3 pt-3 border-t border-dashed border-yellow-500/30 text-center text-sm text-yellow-400"
      >
        {{ $t('settings.log.paused', { count: pendingCount }) }}
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
import { isAndroid } from '@/utils/platform'
import { Pause, Play, Trash2, ArrowDown, PictureInPicture2, Rocket } from 'lucide-vue-next'

// standalone=true 时渲染在独立日志窗口里（隐藏“弹出独立窗口”按钮）
const props = withDefaults(defineProps<{ standalone?: boolean }>(), { standalone: false })

const uiStore = useUIStore()

// 移动端（Android WebView）不支持多窗口，隐藏弹窗相关按钮
const canPopout = !isAndroid()

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

// 等级徽章 / 消息染色（静态类名，保证 Tailwind 能扫描到）
const LEVEL_TAG_CLASSES: Record<string, string> = {
  error: 'text-[#f44747] bg-[rgba(244,71,71,0.14)]',
  warn: 'text-[#e5c07b] bg-[rgba(229,192,123,0.12)]',
  info: 'text-[#98c379] bg-[rgba(152,195,121,0.1)]',
  debug: 'text-[#61afef] bg-[rgba(97,175,239,0.12)]',
  trace: 'text-[#c678dd] bg-[rgba(198,120,221,0.1)]',
}
const MESSAGE_TINT_CLASSES: Record<string, string> = {
  error: 'text-[#fca5a5]!',
  warn: 'text-[#fde68a]!',
  trace: 'text-[rgba(255,255,255,0.45)]!',
}
const levelTagClass = (level: string) => LEVEL_TAG_CLASSES[level.toLowerCase()] ?? ''
const messageTintClass = (level: string) => MESSAGE_TINT_CLASSES[level.toLowerCase()] ?? ''

// 工具栏图标按钮（Tailwind 类，提取常量避免重复）
const ICON_BTN =
  'flex items-center justify-center w-7 h-7 rounded-md border-none bg-white/[0.08] text-white/60 cursor-pointer transition-all duration-200 hover:bg-white/15 hover:text-white'
const ICON_BTN_ACTIVE = 'bg-[rgba(121,217,255,0.2)]! text-(--accent-color,#79d9ff)!'

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

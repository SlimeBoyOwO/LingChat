<template>
  <div
    v-show="visible"
    class="wheel-history-overlay"
    @contextmenu.prevent="close"
    @click="close"
  >
    <div
      class="wheel-history-panel"
      @click.stop
    >
      <div class="wheel-history-header">
        <History :size="20" />
        <h4 class="wheel-history-title">历史对话</h4>
        <span class="wheel-history-hint">右键 / ESC / 框外点击 / 滚到底再下滑 关闭</span>
      </div>

      <div
        v-if="dialogHistory.length === 0"
        class="empty-state"
      >
        暂无历史记录，去和ta聊聊天叭(*^▽^*)
      </div>

      <div v-else class="wheel-history-body">
        <div
          class="wheel-history-content"
          ref="contentRef"
          @wheel="onContentWheel"
        >
          <template v-for="(item, i) in groupedHistory" :key="i">
            <div class="history-block" :class="{ 'is-narration': item.isNarration }">
              <div v-if="!item.isNarration" class="history-name">
                {{ item.displayName }}
              </div>
              <template v-for="(entry, j) in item.lines" :key="j">
                <div
                  v-for="(seg, k) in entry.segments"
                  :key="k"
                  class="history-line"
                  :class="{
                    'is-action': seg.type === 'action',
                    'is-narration-line': item.isNarration && seg.type !== 'action',
                  }"
                >
                  <button
                    v-if="seg.type !== 'action' && entry.audioFile"
                    class="audio-btn"
                    title="播放语音"
                    @click="playAudio(entry.audioFile)"
                  >
                    <Volume2 :size="16" />
                  </button>
                  <span v-if="seg.type === 'action'" class="action-text">{{ seg.text }}</span>
                  <span v-else-if="item.isNarration">{{ seg.text }}</span>
                  <span v-else>{{ '「' + seg.text + '」' }}</span>
                </div>
              </template>
            </div>
          </template>
        </div>
        <div
          v-if="totalPages > 1"
          class="wheel-history-pagination"
        >
          <button
            class="pagination-btn"
            :disabled="currentPage === 1"
            @click="currentPage--"
          >
            上一页
          </button>
          <span class="pagination-info">
            第 {{ currentPage }} 页 / 共 {{ totalPages }} 页
          </span>
          <button
            class="pagination-btn"
            :disabled="currentPage >= totalPages"
            @click="currentPage++"
          >
            下一页
          </button>
        </div>
      </div>
    </div>
    <audio ref="audioRef"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { History, Volume2 } from 'lucide-vue-next'
import type { GameMessage } from '@/stores/modules/game/state'
import { API_CONFIG } from '@/controllers/core/config'

const gameStore = useGameStore()
const uiStore = useUIStore()

const visible = ref(false)
const contentRef = ref<HTMLElement | null>(null)
const audioRef = ref<HTMLAudioElement | null>(null)
const currentPage = ref(1)
const PAGE_SIZE = 100

const dialogHistory = computed<GameMessage[]>(() => gameStore.dialogHistory || [])

const totalPages = computed(() =>
  Math.max(1, Math.ceil(dialogHistory.value.length / PAGE_SIZE))
)

const currentPageHistory = computed(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE
  const end = start + PAGE_SIZE
  return dialogHistory.value.slice(start, end)
})

interface Segment {
  type: 'dialogue' | 'action'
  text: string
}

interface LineEntry {
  segments: Segment[]
  audioFile?: string
}

interface HistoryBlock {
  displayName: string
  isNarration: boolean
  lines: LineEntry[]
}

const narrationNames = new Set(['', '旁白', '系统', 'Narrator', 'System'])

const ACTION_RE = /（[^）]*）/

function stripTrailPeriod(text: string): string {
  return text.replace(/[。]+$/, '')
}

function parseSegments(raw: string, isNarration: boolean): Segment[] {
  const segments: Segment[] = []
  let remaining = raw
  const actions: string[] = []
  let match: RegExpExecArray | null

  while ((match = ACTION_RE.exec(remaining)) !== null) {
    if (match.index > 0) {
      let text = remaining.substring(0, match.index)
      if (!isNarration) text = stripTrailPeriod(text)
      if (text.trim()) segments.push({ type: 'dialogue', text })
    }
    actions.push(match[0])
    remaining = remaining.substring(match.index + match[0].length)
  }

  remaining = remaining.trim()
  if (remaining) {
    if (!isNarration) remaining = stripTrailPeriod(remaining)
    segments.push({ type: 'dialogue', text: remaining })
  }

  if (segments.length === 0 && actions.length > 0) {
    for (const act of actions) {
      segments.push({ type: 'action', text: act })
    }
  } else {
    for (const act of actions) {
      segments.push({ type: 'action', text: act })
    }
  }

  return segments
}

const groupedHistory = computed<HistoryBlock[]>(() => {
  const blocks: HistoryBlock[] = []

  for (const msg of currentPageHistory.value) {
    if (!msg.content || msg.content.trim() === '') continue

    const isNarration = narrationNames.has(msg.displayName || '')

    const name = isNarration
      ? ''
      : msg.displayName || (msg.type === 'message'
          ? (gameStore.userName || gameStore.mainRole?.roleName || '你')
          : '谜之音')

    const segments = parseSegments(msg.content, isNarration)

    const entry: LineEntry = {
      segments,
      audioFile: msg.audioFile,
    }

    const last = blocks.length > 0 ? blocks[blocks.length - 1] : null
    if (last && last.displayName === name && last.isNarration === isNarration) {
      last.lines.push(entry)
    } else {
      blocks.push({ displayName: name, isNarration, lines: [entry] })
    }
  }

  return blocks
})

const isScrolledToBottom = (): boolean => {
  const el = contentRef.value
  if (!el) return true
  return el.scrollHeight - el.scrollTop - el.clientHeight < 30
}

const onContentWheel = (e: WheelEvent) => {
  if (e.deltaY > 0 && isScrolledToBottom()) {
    close()
  }
}

const playAudio = (audioFile: string) => {
  if (!audioFile || !audioRef.value) return
  audioRef.value.src = `${API_CONFIG.VOICE.BASE}/${audioFile}`
  audioRef.value.play()
}

const show = () => {
  if (uiStore.showSettings) return
  visible.value = true
  currentPage.value = totalPages.value
  document.addEventListener('keydown', handleKeyDown)
}

const close = () => {
  visible.value = false
  document.removeEventListener('keydown', handleKeyDown)
}

const handleKeyDown = (e: KeyboardEvent) => {
  if (e.key === 'Escape') {
    close()
  }
}

let wheelAccumulator = 0
const WHEEL_THRESHOLD = 80

const handleWheel = (e: WheelEvent) => {
  if (uiStore.showSettings) return
  if (visible.value) return

  if (e.deltaY < 0) {
    wheelAccumulator += Math.abs(e.deltaY)
    if (wheelAccumulator >= WHEEL_THRESHOLD) {
      wheelAccumulator = 0
      show()
    }
  } else {
    wheelAccumulator = Math.max(0, wheelAccumulator - Math.abs(e.deltaY))
  }
}

defineExpose({ show, close, visible })

onMounted(() => {
  window.addEventListener('wheel', handleWheel, { passive: true })
})

onUnmounted(() => {
  window.removeEventListener('wheel', handleWheel)
  document.removeEventListener('keydown', handleKeyDown)
})
</script>

<style scoped>
.wheel-history-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 2001;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(8px);
}

.wheel-history-panel {
  width: min(95vw, 1100px);
  height: 90vh;
  display: flex;
  flex-direction: column;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 16px 18px;
  backdrop-filter: blur(20px) saturate(180%);
  box-shadow: 0 8px 40px rgba(0, 0, 0, 0.5);
  overflow: hidden;
  color: #f0f0f0;
}

.wheel-history-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-bottom: 8px;
  margin-bottom: 14px;
  border-bottom: 2px solid var(--accent-color, #79d9ff);
  color: #fff;
  flex-shrink: 0;
}

.wheel-history-title {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  text-shadow: 0 0 2px rgba(0, 0, 0, 0.5);
}

.wheel-history-hint {
  margin-left: auto;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.4);
  text-shadow: 0 0 2px rgba(0, 0, 0, 0.5);
}

.wheel-history-body {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.wheel-history-content {
  padding: 14px 6px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  font-size: 21px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scroll-behavior: smooth;
  scrollbar-width: thin;
  scrollbar-color: var(--accent-color, #79d9ff) transparent;
  line-height: 1.9;
}

.history-block {
  padding: 4px 0;
}

.history-block + .history-block {
  border-top: 1px solid rgba(255, 255, 255, 0.07);
  padding-top: 12px;
}

.history-name {
  color: #79d9ff;
  font-weight: 600;
  font-size: 17px;
  margin-bottom: 4px;
}

.history-line {
  color: #e8e8e8;
  word-break: break-word;
  white-space: pre-wrap;
  padding: 3px 0;
  font-size: 21px;
  line-height: 1.9;
  display: flex;
  align-items: flex-start;
  gap: 6px;
}

.is-action {
  color: #c8d0dc;
  font-style: italic;
}

.is-narration {
  border-top: none;
}

.is-narration-line {
  color: #b8c0cc;
  font-style: italic;
}

.action-text {
  color: #c8d0dc;
}

.audio-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  margin-top: 2px;
  border: none;
  border-radius: 4px;
  background: rgba(121, 217, 255, 0.15);
  color: var(--accent-color, #79d9ff);
  cursor: pointer;
  transition: all 0.2s;
}
.audio-btn:hover {
  background: rgba(121, 217, 255, 0.35);
  color: #fff;
}

.wheel-history-pagination {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  width: 100%;
  flex-shrink: 0;
}

.pagination-btn {
  padding: 6px 16px;
  font-size: 14px;
  font-weight: 500;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  background: #e9ecef;
  color: #495057;
  transition: all 0.2s;
}
.pagination-btn:hover:not(:disabled) {
  background: var(--accent-color, #79d9ff);
  color: #fff;
  transform: translateY(-2px);
  box-shadow: 0 4px 10px rgba(121, 217, 255, 0.4);
}
.pagination-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.pagination-info {
  color: #f5f5f5;
  font-size: 16px;
  font-weight: 500;
}

.empty-state {
  text-align: center;
  color: #f5f5f5;
  padding: 40px 0;
  font-size: 24px;
  font-weight: bold;
  text-shadow: 0 0 5px rgba(255, 255, 255, 0.5);
}
</style>

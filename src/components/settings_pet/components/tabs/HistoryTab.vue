<template>
  <article class="w-full h-full flex flex-col min-h-0">
    <!-- 头部区域 -->
    <header class="mb-6 flex items-end justify-between border-b-2 pb-2 transition-colors shrink-0"
      :class="isDarkMode ? 'border-slate-700' : 'border-slate-100'">
      <div>
        <h2 class="text-xl font-black tracking-wide mb-1 transition-colors flex items-center gap-2"
          :class="isDarkMode ? 'text-slate-100' : 'text-slate-800'">
          <History class="w-5 h-5" />
          历史对话
        </h2>
        <p class="text-xs font-medium transition-colors" :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'">
          回顾与ta的过往交流记录吧~
        </p>
      </div>
      <span class="text-4xl font-bold italic select-none font-mono transition-colors"
        :class="isDarkMode ? 'text-slate-700' : 'text-sky-100'">
        02
      </span>
    </header>

    <!-- 主体内容区域 -->
    <div class="flex flex-col flex-1 min-h-0 gap-3">
      <!-- 空状态展示 -->
      <div v-if="dialogHistory.length === 0"
        class="flex-1 flex flex-col items-center justify-center p-8 rounded-xl border-2 border-dashed transition-all"
        :class="isDarkMode
          ? 'bg-slate-800/30 border-slate-700 text-slate-500'
          : 'bg-slate-50 border-slate-200 text-slate-400'
          ">
        <MessageSquare class="w-12 h-12 mb-4 opacity-50" />
        <p class="text-sm font-bold tracking-wider">
          暂无历史记录，去和ta聊聊天叭(*^▽^*)
        </p>
      </div>

      <!-- 历史记录列表 -->
      <div v-else class="flex flex-col flex-1 min-h-0 gap-4">
        <!-- 滚动对话区域 -->
        <div
          ref="contentRef"
          class="flex-1 min-h-0 overflow-y-auto p-4 rounded-xl border shadow-sm transition-all scroll-smooth"
          :class="isDarkMode
            ? 'bg-slate-800/50 border-slate-700'
            : 'bg-white border-slate-200'
          "
          style="line-height: 1.9; font-size: 18px"
        >
          <template v-for="(item, i) in groupedHistory" :key="i">
            <div
              class="py-1"
              :class="{ 'border-t pt-3 mt-0': !item.isNarration && i > 0 }"
              :style="isDarkMode ? 'border-color: rgba(255,255,255,0.1)' : 'border-color: rgba(0,0,0,0.06)'"
            >
              <div v-if="!item.isNarration" class="mb-1 flex items-center justify-between">
                <span
                  class="text-[17px] font-semibold transition-colors"
                  :class="isDarkMode ? 'text-sky-400' : 'text-sky-600'"
                >
                  {{ item.displayName }}
                </span>
                <button
                  v-if="item.userMessageSeq !== undefined"
                  class="shrink-0 cursor-pointer rounded border border-white/10 bg-transparent px-2 py-0.5 text-xs text-white/40 transition-all duration-200 hover:border-red-400/50 hover:bg-red-500/20 hover:text-white"
                  title="回溯到此消息之前（将清除此消息及之后所有对话）"
                  @click.stop="handleBacktrack(item.userMessageSeq!)"
                >
                  回溯
                </button>
              </div>
              <template v-for="(entry, j) in item.lines" :key="j">
                <div
                  v-for="(seg, k) in entry.segments"
                  :key="k"
                  class="flex items-start gap-1.5 py-0.5 whitespace-pre-wrap wrap-break-word"
                  :class="{
                    'italic': seg.type === 'action' || item.isNarration,
                  }"
                  :style="{
                    color: seg.type === 'action'
                      ? (isDarkMode ? '#c8d0dc' : '#64748b')
                      : item.isNarration
                        ? (isDarkMode ? '#b8c0cc' : '#475569')
                        : (isDarkMode ? '#e8e8e8' : '#1e293b'),
                    fontSize: '18px',
                    lineHeight: '1.9',
                  }"
                >
                  <span v-if="seg.type === 'action'">{{ seg.text }}</span>
                  <span v-else-if="item.isNarration">{{ seg.text }}</span>
                  <span v-else>{{ '「' + seg.text + '」' }}</span>
                  <button
                    v-if="seg.type !== 'action' && entry.audioFile"
                    class="mt-0.5 inline-flex h-5.5 w-5.5 shrink-0 cursor-pointer items-center justify-center rounded border-0 transition-all duration-200"
                    :class="isDarkMode
                      ? 'bg-[rgba(121,217,255,0.15)] text-sky-400 hover:bg-[rgba(121,217,255,0.35)] hover:text-white'
                      : 'bg-sky-100 text-sky-600 hover:bg-sky-200 hover:text-sky-800'"
                    title="播放语音"
                    @click="playAudio(entry.audioFile)"
                  >
                    <Volume2 :size="16" />
                  </button>
                </div>
              </template>
            </div>
          </template>
        </div>

        <!-- 分页控制器 -->
        <div v-if="totalPages > 1" class="flex items-center justify-between px-1 shrink-0">
          <button
            class="px-4 py-2 text-xs font-bold rounded-lg transition-all flex items-center gap-1 border cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
            :class="isDarkMode
              ? 'bg-slate-800/50 text-slate-300 border-slate-700 hover:bg-slate-700 hover:border-slate-600 hover:text-sky-400'
              : 'bg-white text-slate-600 border-slate-200 hover:bg-sky-50 hover:border-sky-200 hover:text-sky-500'
            "
            :disabled="currentPage === 1"
            @click="currentPage--"
          >
            <ChevronLeft class="w-4 h-4" /> 上一页
          </button>

          <span
            class="text-xs font-bold tracking-widest font-mono transition-colors"
            :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
          >
            第 {{ currentPage }} 页 / 共 {{ totalPages }} 页
          </span>

          <button
            class="px-4 py-2 text-xs font-bold rounded-lg transition-all flex items-center gap-1 border cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
            :class="isDarkMode
              ? 'bg-slate-800/50 text-slate-300 border-slate-700 hover:bg-slate-700 hover:border-slate-600 hover:text-sky-400'
              : 'bg-white text-slate-600 border-slate-200 hover:bg-sky-50 hover:border-sky-200 hover:text-sky-500'
            "
            :disabled="currentPage >= totalPages"
            @click="currentPage++"
          >
            下一页
            <ChevronRight class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>

    <audio ref="audioRef"></audio>
  </article>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, nextTick } from 'vue'
import {
  History,
  MessageSquare,
  ChevronLeft,
  ChevronRight,
  Volume2,
} from 'lucide-vue-next'
import { useGameStore } from '../../../../stores/modules/game'
import type { GameMessage } from '../../../../stores/modules/game/state'
import { convertInitLines } from '../../../../stores/modules/game/actions'
import { useDialogStore } from '../../../../stores/modules/ui/dialog'
import { getVoiceAudio } from '@/api/services/game-info'
import { invoke } from '@tauri-apps/api/core'
import type { GameLineInit } from '@/api/services/game-info'

// --- Props ---
defineProps<{
  isDarkMode: boolean
}>()

// --- 类型定义 ---
interface Segment {
  type: 'dialogue' | 'action'
  text: string
}

interface LineEntry {
  segments: Segment[]
  audioFile?: string
  userMessageSeq?: number
}

interface HistoryBlock {
  displayName: string
  isNarration: boolean
  lines: LineEntry[]
  userMessageSeq?: number
}

// --- Store & Refs ---
const gameStore = useGameStore()
const dialogStore = useDialogStore()
const audioRef = ref<HTMLAudioElement>()
const contentRef = ref<HTMLDivElement>()

const dialogHistory = computed<GameMessage[]>(() => gameStore.dialogHistory)
const narrationNames = new Set(['', '旁白', '系统', 'Narrator', 'System'])
const ACTION_RE = /（[^）]*）/

// --- 分页 ---
const PAGE_SIZE = 100
const currentPage = ref(1)
const totalPages = computed(() => Math.ceil(dialogHistory.value.length / PAGE_SIZE))

const currentPageHistory = computed(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE
  const end = start + PAGE_SIZE
  return dialogHistory.value.slice(start, end)
})

// --- 分组历史（与 SettingsHistory 同步逻辑）---
const groupedHistory = computed<HistoryBlock[]>(() => {
  const blocks: HistoryBlock[] = []

  for (const msg of currentPageHistory.value) {
    if (!msg.content || msg.content.trim() === '') continue

    const isNarration = narrationNames.has(msg.displayName || '')

    const name = isNarration
      ? ''
      : msg.displayName ||
        (msg.type === 'message'
          ? gameStore.userName || gameStore.mainRole?.roleName || '你'
          : '谜之音')

    const segments = parseSegments(msg.content, msg.motionText, isNarration)

    const entry: LineEntry = {
      segments,
      audioFile: msg.audioFile,
      userMessageSeq: msg.userMessageSeq,
    }

    const last = blocks.length > 0 ? blocks[blocks.length - 1] : null
    if (last && last.displayName === name && last.isNarration === isNarration) {
      if (entry.userMessageSeq !== undefined && last.userMessageSeq === undefined) {
        last.userMessageSeq = entry.userMessageSeq
      }
      last.lines.push(entry)
    } else {
      blocks.push({
        displayName: name,
        isNarration,
        lines: [entry],
        userMessageSeq: entry.userMessageSeq,
      })
    }
  }

  return blocks
})

// --- 分段解析（与 SettingsHistory 同步逻辑）---
function stripTrailPeriod(text: string): string {
  return text.replace(/[。]+$/, '')
}

function parseSegments(
  raw: string,
  actionPart: string | undefined,
  isNarration: boolean,
): Segment[] {
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

  if (actionPart) {
    segments.push({ type: 'action', text: actionPart })
  }

  return segments
}

// --- 回溯（与 SettingsHistory 同步逻辑）---
async function handleBacktrack(messageSeq: number) {
  const confirmed = await dialogStore.confirm(
    '确定要回溯到此对话吗？此操作将清除该消息及之后的所有对话，且不可撤销。',
    '回溯确认',
  )
  if (!confirmed) return

  try {
    const lines = await invoke<any[]>('rollback_conversation', {
      messageSeq,
    })

    const messages = convertInitLines(
      lines.map(
        (l: any): GameLineInit => ({
          content: l.content,
          attribute: l.attribute,
          sender_role_id: l.sender_role_id,
          display_name: l.display_name,
          original_emotion: l.original_emotion,
          predicted_emotion: l.predicted_emotion,
          action_content: l.action_content,
          audio_file: l.audio_file,
          perceived_role_ids: l.perceived_role_ids,
          user_message_seq: l.user_message_seq,
        }),
      ),
    )

    gameStore.setGameMessages(messages)
  } catch (error: any) {
    console.error('回溯对话失败:', error)
    await dialogStore.alert('回溯失败：' + (typeof error === 'string' ? error : error.message))
  }
}

// --- 音频播放（与 SettingsHistory 同步逻辑）---
const playAudio = async (audioFile: string) => {
  if (!audioFile || !audioRef.value) return
  audioRef.value.src = await getVoiceAudio(audioFile)
  audioRef.value.play()
}

// --- 滚动到内容底部 ---
async function scrollToBottom() {
  await nextTick()
  if (contentRef.value) {
    contentRef.value.scrollTop = contentRef.value.scrollHeight
  }
}

// --- 生命周期 ---
onMounted(async () => {
  if (dialogHistory.value.length > 0) {
    currentPage.value = totalPages.value
    await scrollToBottom()
  }
})

// 切换页码时滚动到顶部
watch(currentPage, () => {
  if (contentRef.value) {
    contentRef.value.scrollTop = 0
  }
})

// 当切换到最后一页时自动滚动到底部
watch([currentPage, groupedHistory], async () => {
  if (currentPage.value === totalPages.value) {
    await scrollToBottom()
  }
})

// 监听对话历史变化，跳转到最后一页（最新记录）
watch(
  dialogHistory,
  () => {
    currentPage.value = totalPages.value
  },
  { deep: true },
)
</script>

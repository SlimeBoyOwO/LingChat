<template>
  <MenuPage>
    <MenuItem title="历史对话">
      <template #header>
        <History :size="20" />
      </template>
      <div class="flex flex-col h-full min-h-0">
        <div v-if="dialogHistory.length === 0" class="flex flex-1 items-center justify-center">
          <div
            class="py-10 text-center text-2xl font-bold text-gray-100 [text-shadow:0_0_5px_rgba(255,255,255,0.5)]"
          >
            暂无历史记录，去和ta聊聊天叭(*^▽^*)
          </div>
        </div>

        <div v-else class="flex flex-1 flex-col min-h-0">
          <div
            ref="contentRef"
            class="flex-1 min-h-0 overflow-y-auto px-1.5 py-3.5 scrollbar-thin [scrollbar-color:var(--accent-color,#79d9ff)_transparent] scroll-smooth"
            style="line-height: 1.9; font-size: 18px"
          >
            <template v-for="(item, i) in groupedHistory" :key="i">
              <div
                class="py-1"
                :class="{ 'border-t border-white/10 pt-3 mt-0': !item.isNarration && i > 0 }"
              >
                <div v-if="!item.isNarration" class="mb-1 text-[17px] font-semibold text-[#79d9ff]">
                  {{ item.displayName }}
                </div>
                <template v-for="(entry, j) in item.lines" :key="j">
                  <div
                    v-for="(seg, k) in entry.segments"
                    :key="k"
                    class="flex items-start gap-1.5 py-0.5 whitespace-pre-wrap wrap-break-word"
                    :class="{
                      'text-[#c8d0dc] italic': seg.type === 'action',
                      'text-[#b8c0cc] italic': item.isNarration && seg.type !== 'action',
                      'text-[#e8e8e8]': seg.type !== 'action' && !item.isNarration,
                    }"
                    style="font-size: 18px; line-height: 1.9"
                  >
                    <span v-if="seg.type === 'action'" class="text-[#c8d0dc]">{{ seg.text }}</span>
                    <span v-else-if="item.isNarration">{{ seg.text }}</span>
                    <span v-else>{{ '「' + seg.text + '」' }}</span>
                    <button
                      v-if="seg.type !== 'action' && entry.audioFile"
                      class="mt-0.5 inline-flex h-5.5 w-5.5 shrink-0 cursor-pointer items-center justify-center rounded border-0 bg-[rgba(121,217,255,0.15)] text-(--accent-color,#79d9ff) transition-all duration-200 hover:bg-[rgba(121,217,255,0.35)] hover:text-white"
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

          <div
            v-if="totalPages > 1"
            class="mt-auto flex w-full shrink-0 items-center justify-between px-3 py-2"
          >
            <button
              class="cursor-pointer rounded-lg border-0 bg-[#e9ecef] px-4 py-1.5 text-sm font-medium text-[#495057] transition-all duration-200 disabled:cursor-not-allowed disabled:opacity-40 hover:not-disabled:bg-(--accent-color,#79d9ff) hover:not-disabled:text-white hover:not-disabled:-translate-y-0.5 hover:not-disabled:shadow-[0_4px_10px_rgba(121,217,255,0.4)]"
              :disabled="currentPage === 1"
              @click="currentPage--"
            >
              上一页
            </button>
            <span class="text-base font-medium text-gray-100">
              第 {{ currentPage }} 页 / 共 {{ totalPages }} 页
            </span>
            <button
              class="cursor-pointer rounded-lg border-0 bg-[#e9ecef] px-4 py-1.5 text-sm font-medium text-[#495057] transition-all duration-200 disabled:cursor-not-allowed disabled:opacity-40 hover:not-disabled:bg-(--accent-color,#79d9ff) hover:not-disabled:text-white hover:not-disabled:-translate-y-0.5 hover:not-disabled:shadow-[0_4px_10px_rgba(121,217,255,0.4)]"
              :disabled="currentPage >= totalPages"
              @click="currentPage++"
            >
              下一页
            </button>
          </div>

          <audio ref="audioRef"></audio>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
// 1. 从 vue 中引入 ref 和 watch
import { ref, computed, watch } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import { useGameStore } from '../../../stores/modules/game'
import type { GameMessage } from '../../../stores/modules/game/state'
import { History, Volume2 } from 'lucide-vue-next'
import { getVoiceAudio } from '@/api/services/game-info'

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

const gameStore = useGameStore()
const audioRef = ref<HTMLAudioElement>()

const dialogHistory = computed<GameMessage[]>(() => gameStore.dialogHistory)
const narrationNames = new Set(['', '旁白', '系统', 'Narrator', 'System'])
const ACTION_RE = /（[^）]*）/

// 每页显示的台词数量
const PAGE_SIZE = 100

// 当前页码
const currentPage = ref(1)

// 计算总页数
const totalPages = computed(() => Math.ceil(dialogHistory.value.length / PAGE_SIZE))

// 计算当前页应该显示的对话历史
const currentPageHistory = computed(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE
  const end = start + PAGE_SIZE
  return dialogHistory.value.slice(start, end)
})

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

const playAudio = async (audioFile: string) => {
  if (!audioFile || !audioRef.value) return
  audioRef.value.src = await getVoiceAudio(audioFile)
  audioRef.value.play()
}

// 监听对话历史变化，重置到第一页
watch(
  dialogHistory,
  () => {
    currentPage.value = 1
  },
  { deep: true },
)

// 对话初始化逻辑在 gameStore 的初始化中处理
</script>

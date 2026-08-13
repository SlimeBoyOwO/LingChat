<template>
  <div class="flex flex-col gap-3">
    <Button
      type="nav"
      :class="[
        'flex items-center gap-2 px-4 py-2 transition-colors',
        enabled ? 'text-[#4facfe]' : 'text-white',
      ]"
      @click="toggleEnabled"
      v-show="!uiStore.showSettings"
    >
      <span class="text-xl">🍅</span>
      <h3 class="text-lg font-bold m-0 hidden xl:block">
        {{ $t('ui.pomodoro.title') }}
        <span v-if="isRunning" class="ml-1 text-sm font-normal tabular-nums opacity-80">
          {{ minutes }}:{{ seconds }}
        </span>
      </h3>
    </Button>

    <Transition
      enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      enter-from-class="opacity-0 -translate-y-2"
      leave-to-class="opacity-0 -translate-y-2"
    >
      <div
        v-if="enabled"
        class="w-65 bg-[#12121c]/75 backdrop-blur-[20px] border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)] rounded-3xl p-6 text-white flex flex-col items-center box-border"
      >
        <div class="relative mb-6 outline-none">
          <div class="w-45 h-45 relative outline-none border-none">
            <svg
              class="w-full h-full -rotate-90 outline-none overflow-visible block"
              viewBox="0 0 100 100"
            >
              <defs>
                <linearGradient id="gradient-ring" x1="0%" y1="0%" x2="100%" y2="100%">
                  <stop offset="0%" stop-color="#4facfe" />
                  <stop offset="100%" stop-color="#00f2fe" />
                </linearGradient>
              </defs>
              <circle class="fill-none stroke-white/10 stroke-4" cx="50" cy="50" r="45" />
              <circle
                class="fill-none stroke-[url(#gradient-ring)] stroke-4 stroke-round transition-[stroke-dashoffset] duration-1000 ease-linear drop-shadow-[0_0_4px_rgba(79,172,254,0.5)]"
                cx="50"
                cy="50"
                r="45"
                :style="progressStyle"
              />
              <!-- 隐形加宽轨道：圆环的拖动热区（不显示拖动手柄） -->
              <circle
                class="fill-none stroke-transparent cursor-pointer"
                style="stroke-width: 14; touch-action: none"
                cx="50"
                cy="50"
                r="45"
                @pointerdown="onScrubStart"
              />
            </svg>

            <div class="absolute inset-0 flex flex-col items-center justify-center z-10 pointer-events-none">
              <div
                class="h-6 flex items-center justify-center mb-1 cursor-pointer group pointer-events-auto"
                @click="startEditLabel"
                :title="$t('ui.pomodoro.editName')"
              >
                <span
                  v-if="!editingLabel"
                  class="text-base font-medium tracking-wide opacity-90 group-hover:text-[#4facfe] transition-colors"
                >
                  {{ workLabel }}
                </span>
                <input
                  v-else
                  v-model="workLabelDraft"
                  class="w-30 bg-transparent border-0 border-b border-[#4facfe] text-white text-center text-base outline-none p-0 focus:ring-0"
                  @blur="commitEditLabel"
                  @keyup.enter="commitEditLabel"
                  autofocus
                />
              </div>

              <div
                class="text-5xl font-bold leading-none tabular-nums my-1 drop-shadow-[0_4px_12px_rgba(0,0,0,0.3)]"
              >
                {{ minutes }}:{{ seconds }}
              </div>

              <div class="text-[13px] text-[#4facfe] font-semibold mb-0.5">
                {{ statusText }}
              </div>
              <div class="text-[11px] text-white/50">
                {{ $t('ui.pomodoro.cycleInfo', { current: cycleIndex, total: cyclesTotal }) }}
              </div>
            </div>
          </div>
        </div>

        <div class="flex items-center justify-between w-40 mb-6">
          <div
            class="w-12 h-12 rounded-full bg-white/10 flex items-center justify-center cursor-pointer transition-all duration-200 text-white hover:bg-white/20 hover:scale-105 active:scale-95"
            :class="{ 'opacity-30 pointer-events-none bg-transparent shadow-none': isRunning }"
            @click="start"
            :title="$t('ui.pomodoro.start')"
          >
            <svg viewBox="0 0 24 24" width="28" height="28" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
          </div>

          <div
            class="w-12 h-12 rounded-full bg-white/10 flex items-center justify-center cursor-pointer transition-all duration-200 text-white hover:bg-white/20 hover:scale-105 active:scale-95"
            :class="{ 'opacity-30 pointer-events-none bg-transparent shadow-none': !isRunning }"
            @click="pause"
            :title="$t('ui.pomodoro.pause')"
          >
            <svg viewBox="0 0 24 24" width="28" height="28" fill="currentColor">
              <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" />
            </svg>
          </div>

          <div
            class="w-12 h-12 rounded-full bg-white/10 flex items-center justify-center cursor-pointer transition-all duration-200 text-white hover:bg-white/20 hover:scale-105 active:scale-95"
            @click="reset"
            :title="$t('ui.pomodoro.reset')"
          >
            <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
              <path
                d="M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"
              />
            </svg>
          </div>
        </div>

        <div class="flex justify-between w-full pt-4 border-t border-white/10">
          <div class="flex flex-col items-center flex-1">
            <span class="text-[11px] text-white/50 mb-1.5">{{ $t('ui.pomodoro.work') }}</span>
            <div class="flex items-center justify-center relative h-6">
              <input
                type="number"
                class="no-spin w-8 bg-transparent border-none text-white text-right font-medium text-[15px] outline-none p-0 appearance-none"
                v-model.number="workMinutesInput"
                @change="applyWorkMinutes"
              />
              <span class="text-[11px] text-white/50 pointer-events-none ml-0.5 mr-1">m</span>
              <div class="flex flex-col justify-center h-full gap-0.5">
                <div
                  class="flex items-center justify-center cursor-pointer opacity-60 hover:opacity-100 h-2.5 active:scale-90 transition-transform"
                  @click="adjustWork(1)"
                >
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 14l5-5 5 5z" />
                  </svg>
                </div>
                <div
                  class="flex items-center justify-center cursor-pointer opacity-60 hover:opacity-100 h-2.5 active:scale-90 transition-transform"
                  @click="adjustWork(-1)"
                >
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 10l5 5 5-5z" />
                  </svg>
                </div>
              </div>
            </div>
          </div>

          <div class="flex flex-col items-center flex-1">
            <span class="text-[11px] text-white/50 mb-1.5">{{ $t('ui.pomodoro.break') }}</span>
            <div class="flex items-center justify-center relative h-6">
              <input
                type="number"
                class="no-spin w-8 bg-transparent border-none text-white text-right font-medium text-[15px] outline-none p-0 appearance-none"
                v-model.number="breakMinutesInput"
                @change="applyBreakMinutes"
              />
              <span class="text-[11px] text-white/50 pointer-events-none ml-0.5 mr-1">m</span>
              <div class="flex flex-col justify-center h-full gap-0.5">
                <div
                  class="flex items-center justify-center cursor-pointer opacity-60 hover:opacity-100 h-2.5 active:scale-90 transition-transform"
                  @click="adjustBreak(1)"
                >
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 14l5-5 5 5z" />
                  </svg>
                </div>
                <div
                  class="flex items-center justify-center cursor-pointer opacity-60 hover:opacity-100 h-2.5 active:scale-90 transition-transform"
                  @click="adjustBreak(-1)"
                >
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 10l5 5 5-5z" />
                  </svg>
                </div>
              </div>
            </div>
          </div>

          <div class="flex flex-col items-center flex-1">
            <span class="text-[11px] text-white/50 mb-1.5">{{ $t('ui.pomodoro.cycles') }}</span>
            <div class="flex items-center justify-center relative h-6">
              <input
                type="number"
                class="no-spin w-8 bg-transparent border-none text-white text-right font-medium text-[15px] outline-none p-0 appearance-none"
                v-model.number="cyclesInput"
                @change="applyCycles"
              />
              <span class="text-[11px] text-white/50 pointer-events-none ml-0.5 mr-1">{{ $t('ui.pomodoro.cycleUnit') }}</span>
              <div class="flex flex-col justify-center h-full gap-0.5">
                <div
                  class="flex items-center justify-center cursor-pointer opacity-60 hover:opacity-100 h-2.5 active:scale-90 transition-transform"
                  @click="adjustCycles(1)"
                >
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 14l5-5 5 5z" />
                  </svg>
                </div>
                <div
                  class="flex items-center justify-center cursor-pointer opacity-60 hover:opacity-100 h-2.5 active:scale-90 transition-transform"
                  @click="adjustCycles(-1)"
                >
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 10l5 5 5-5z" />
                  </svg>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import Button from '../base/widget/Button.vue'
import { useGameStore } from '../../stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const gameStore = useGameStore()
const uiStore = useUIStore()

const STORAGE_KEY_ENABLED = 'pomodoro_enabled'
const STORAGE_KEY_REMAINING = 'pomodoro_remaining_ms'
const STORAGE_KEY_RUNNING = 'pomodoro_running'
const STORAGE_KEY_MODE = 'pomodoro_mode'
const STORAGE_KEY_CYCLE_INDEX = 'pomodoro_cycle_idx'
const STORAGE_KEY_CYCLES_TOTAL = 'pomodoro_cycles_total'
const STORAGE_KEY_WORK_MS = 'pomodoro_work_ms'
const STORAGE_KEY_BREAK_MS = 'pomodoro_break_ms'
const STORAGE_KEY_WORK_LABEL = 'pomodoro_work_label'
const STORAGE_KEY_PHASE_END_AT = 'pomodoro_phase_end_at'
const STORAGE_KEY_COMPLETED = 'pomodoro_completed'

type Mode = 'work' | 'break'

const DEFAULT_WORK_MS = 25 * 60 * 1000
const DEFAULT_BREAK_MS = 5 * 60 * 1000
const DEFAULT_CYCLES_TOTAL = 2

const enabled = ref(false)
const isRunning = ref(false)
const mode = ref<Mode>('work')
// 用户自定义的专注标签；为空时跟随界面语言显示默认文案
const customWorkLabel = ref('')
const workLabel = computed(() => customWorkLabel.value || t('ui.pomodoro.defaultWorkLabel'))
const editingLabel = ref(false)
const workLabelDraft = ref('')

const workDurationMs = ref<number>(DEFAULT_WORK_MS)
const breakDurationMs = ref<number>(DEFAULT_BREAK_MS)
const cyclesTotal = ref<number>(DEFAULT_CYCLES_TOTAL)
const cycleIndex = ref<number>(1)

const remainingMs = ref<number>(DEFAULT_WORK_MS)
// 当前阶段的结束时刻（时间戳，毫秒）。运行中以它为准计算剩余时间，
// 页面被节流/挂起/刷新后可以用 Date.now() 一次性补跑，不会丢时间。
const phaseEndAt = ref<number>(0)
// 全部轮次已完成（对应参考游戏的 Complete 状态），下次开始会从第一轮专注重新起步
const justCompleted = ref(false)
let timerId: number | null = null

const workMinutesInput = ref(25)
const breakMinutesInput = ref(5)
const cyclesInput = ref(2)

const currentTotalMs = computed(() =>
  mode.value === 'work' ? workDurationMs.value : breakDurationMs.value,
)

const minutes = computed(() => {
  const m = Math.floor(remainingMs.value / 60000)
  return m.toString().padStart(2, '0')
})
const seconds = computed(() => {
  const s = Math.floor((remainingMs.value % 60000) / 1000)
  return s.toString().padStart(2, '0')
})

const circumference = 2 * Math.PI * 45
const progress = computed(() => {
  const total = Math.max(1, currentTotalMs.value)
  const p = 1 - remainingMs.value / total
  return Math.min(1, Math.max(0, p))
})
// 正在拖动圆环调整时间（不显示拖动手柄，圆环本身就是热区）
const scrubbing = ref(false)

const progressStyle = computed(() => ({
  strokeDasharray: `${circumference}`,
  strokeDashoffset: `${(1 - progress.value) * circumference}`,
  transformOrigin: '50% 50%',
  // 拖动时禁用过渡动画，否则圆环跟不上指针
  transition: scrubbing.value ? 'none' : undefined,
}))

const statusText = computed(() => {
  if (isRunning.value) {
    return mode.value === 'work' ? t('ui.pomodoro.statusWorking') : t('ui.pomodoro.statusBreaking')
  }
  if (justCompleted.value) {
    return t('ui.pomodoro.statusCompleted')
  }
  // 从未启动过的初始状态才算空闲，停在半途中属于暂停
  const isPristine =
    remainingMs.value === currentTotalMs.value && cycleIndex.value === 1 && mode.value === 'work'
  return isPristine ? t('ui.pomodoro.statusIdle') : t('ui.pomodoro.statusPaused')
})

const pendingPrompts = ref<string[]>([])

function formatMinutes(ms: number) {
  return Math.max(1, Math.round(ms / 60000))
}

function sendUserPrompt(text: string) {
  const content = (text || '').trim()
  if (!content) return

  if (gameStore.currentStatus !== 'input') {
    pendingPrompts.value.push(content)
    return
  }

  gameStore.currentStatus = 'thinking'
  // 记录用户消息的插入位置，失败时回退要把它从聊天里撤掉
  const userMessageIndex = gameStore.dialogHistory.length
  gameStore.appendGameMessage({
    type: 'message',
    displayName: gameStore.userName,
    content,
  })
  invoke('send_chat_message', { text: content })
    .then(() => {
      // 发送成功，状态由后端事件驱动更新
    })
    .catch((error) => {
      console.error('发送消息失败:', error)
      // 移除孤立的用户消息，避免在聊天里留下无回复的尾巴
      // 仅当那条消息仍是用户发出的原内容时才删除，防止误删后端并发写入
      const last = gameStore.dialogHistory[userMessageIndex]
      if (last && last.type === 'message' && last.content === content) {
        gameStore.dialogHistory.splice(userMessageIndex, 1)
      }
      gameStore.currentStatus = 'input'
    })
}

function flushPendingPrompts() {
  if (pendingPrompts.value.length === 0) return
  if (gameStore.currentStatus !== 'input') return
  const next = pendingPrompts.value.shift()
  if (next) sendUserPrompt(next)
}

watch(
  () => gameStore.currentStatus,
  (status) => {
    if (status === 'input') flushPendingPrompts()
  },
)

watch(
  () => uiStore.showSettings,
  (show) => {
    if (show) enabled.value = false
  },
)

function persistState() {
  localStorage.setItem(STORAGE_KEY_ENABLED, JSON.stringify(enabled.value))
  localStorage.setItem(STORAGE_KEY_REMAINING, JSON.stringify(remainingMs.value))
  localStorage.setItem(STORAGE_KEY_RUNNING, JSON.stringify(isRunning.value))
  localStorage.setItem(STORAGE_KEY_MODE, mode.value)
  localStorage.setItem(STORAGE_KEY_CYCLE_INDEX, JSON.stringify(cycleIndex.value))
  localStorage.setItem(STORAGE_KEY_CYCLES_TOTAL, JSON.stringify(cyclesTotal.value))
  localStorage.setItem(STORAGE_KEY_WORK_MS, JSON.stringify(workDurationMs.value))
  localStorage.setItem(STORAGE_KEY_BREAK_MS, JSON.stringify(breakDurationMs.value))
  localStorage.setItem(STORAGE_KEY_WORK_LABEL, customWorkLabel.value)
  localStorage.setItem(STORAGE_KEY_PHASE_END_AT, JSON.stringify(phaseEndAt.value))
  localStorage.setItem(STORAGE_KEY_COMPLETED, JSON.stringify(justCompleted.value))
}

function clearTimer() {
  if (timerId) {
    clearInterval(timerId)
    timerId = null
  }
}

// ─── 圆环拖动调时间 ─────────────────────────────────
// 指针位置 → 从 12 点方向顺时针的进度比例（与圆环绘制方向一致）
let scrubEl: Element | null = null

function scrubFraction(e: PointerEvent) {
  if (!scrubEl) return 0
  const rect = scrubEl.getBoundingClientRect()
  const dx = e.clientX - (rect.left + rect.width / 2)
  const dy = e.clientY - (rect.top + rect.height / 2)
  return (Math.atan2(dx, -dy) / (2 * Math.PI) + 1) % 1
}

function applyScrub(e: PointerEvent) {
  const total = currentTotalMs.value
  // 圆环进度 = 1 - 剩余/总时长；限制在 1 秒 ~ 总时长之间，避免误触阶段切换
  const next = Math.round(((1 - scrubFraction(e)) * total) / 1000) * 1000
  remainingMs.value = Math.min(total, Math.max(1000, next))
  if (isRunning.value) {
    phaseEndAt.value = Date.now() + remainingMs.value
  }
}

function onScrubStart(e: PointerEvent) {
  scrubEl = e.currentTarget as Element
  scrubbing.value = true
  applyScrub(e)
  window.addEventListener('pointermove', onScrubMove)
  window.addEventListener('pointerup', onScrubEnd, { once: true })
  e.preventDefault()
}

function onScrubMove(e: PointerEvent) {
  if (!scrubbing.value) return
  applyScrub(e)
}

function onScrubEnd() {
  if (!scrubbing.value) return
  scrubbing.value = false
  scrubEl = null
  window.removeEventListener('pointermove', onScrubMove)
  persistState()
}

// 推进到下一个阶段，返回需要发给 AI 的提示文本。
// 下一阶段的结束时刻沿用上一次的结束时间链式推算，长时间挂起后可以逐阶段补跑。
function advancePhase(): string | null {
  const prevCycle = cycleIndex.value
  const endedAt = phaseEndAt.value

  if (mode.value === 'work') {
    mode.value = 'break'
    phaseEndAt.value = endedAt + breakDurationMs.value
    return `{番茄钟提醒：第${prevCycle}/${cyclesTotal.value}轮专注结束，开始休息 ${formatMinutes(breakDurationMs.value)} 分钟。}`
  }

  if (cycleIndex.value < cyclesTotal.value) {
    cycleIndex.value += 1
    mode.value = 'work'
    phaseEndAt.value = endedAt + workDurationMs.value
    return `{番茄钟提醒：休息结束，开始第${cycleIndex.value}/${cyclesTotal.value}轮专注（${workLabel.value}），时长 ${formatMinutes(workDurationMs.value)} 分钟}`
  }

  // 所有轮次完成：回到第一轮专注的待启动状态，下次按开始才能正确从专注起步
  clearTimer()
  isRunning.value = false
  justCompleted.value = true
  mode.value = 'work'
  cycleIndex.value = 1
  remainingMs.value = workDurationMs.value
  phaseEndAt.value = 0
  return `{番茄钟提醒：本次番茄钟已完成（专注 ${formatMinutes(workDurationMs.value)} 分钟 + 休息 ${formatMinutes(breakDurationMs.value)} 分钟 × ${cyclesTotal.value} 轮）。}`
}

function tick() {
  if (!isRunning.value || phaseEndAt.value <= 0) return

  const now = Date.now()
  let remaining = phaseEndAt.value - now
  const prompts: string[] = []
  let guard = 0

  // 页面被节流/挂起/刷新时 remaining 可能已跨过多个阶段，循环补跑直到追上当前时刻
  while (remaining <= 0 && guard++ < 10000) {
    const prompt = advancePhase()
    if (prompt) prompts.push(prompt)
    if (!isRunning.value) break // 全部轮次完成
    remaining = phaseEndAt.value - now
  }

  if (isRunning.value) {
    remainingMs.value = Math.max(0, remaining)
  }

  // 补跑跨过了多个阶段时只发最后一条提示，避免恢复时连续刷屏
  const lastPrompt = prompts[prompts.length - 1]
  if (lastPrompt) sendUserPrompt(lastPrompt)

  persistState()
}

function start() {
  if (isRunning.value) return
  // 已完成或剩余时间异常归零时，从第一轮专注重新开始，
  // 否则会错误地以上一次结束时的休息阶段启动
  if (justCompleted.value || remainingMs.value <= 0) {
    mode.value = 'work'
    cycleIndex.value = 1
    remainingMs.value = workDurationMs.value
  }
  // 中途继续和全新启动发给 AI 的提示不同（全新启动的消息格式被成就系统识别，不可改动）
  const isResume = remainingMs.value > 0 && remainingMs.value < currentTotalMs.value
  justCompleted.value = false
  isRunning.value = true
  phaseEndAt.value = Date.now() + remainingMs.value
  clearTimer()
  timerId = window.setInterval(tick, 1000)
  persistState()

  if (isResume) {
    const resumePhase = mode.value === 'work' ? `专注（${workLabel.value}）` : '休息'
    sendUserPrompt(
      `{番茄钟提醒：我继续番茄钟，第${cycleIndex.value}/${cyclesTotal.value}轮${resumePhase}，剩余 ${minutes.value}:${seconds.value}。}`,
    )
    return
  }

  const phaseText = mode.value === 'work' ? `开始专注（${workLabel.value}）` : '开始休息'
  sendUserPrompt(
    `{我启动了番茄钟：专注 ${formatMinutes(workDurationMs.value)} 分钟，休息 ${formatMinutes(breakDurationMs.value)} 分钟，共 ${cyclesTotal.value} 轮。现在${phaseText}，这是第${cycleIndex.value}/${cyclesTotal.value}轮。}`,
  )
}

function pause() {
  if (!isRunning.value) return
  // 把剩余时间落盘为固定值，恢复时再折算成新的结束时刻
  remainingMs.value = Math.max(0, phaseEndAt.value - Date.now())
  phaseEndAt.value = 0
  isRunning.value = false
  clearTimer()
  persistState()

  // 暂停也告知 AI，方便角色做出反应（与阶段切换提醒同格式）
  const phaseText = mode.value === 'work' ? `专注（${workLabel.value}）` : '休息'
  sendUserPrompt(
    `{番茄钟提醒：我暂停了番茄钟，第${cycleIndex.value}/${cyclesTotal.value}轮${phaseText}，剩余 ${minutes.value}:${seconds.value}。}`,
  )
}

function reset() {
  mode.value = 'work'
  cycleIndex.value = 1
  remainingMs.value = workDurationMs.value
  justCompleted.value = false
  phaseEndAt.value = 0
  isRunning.value = false
  clearTimer()
  persistState()
}

function toggleEnabled() {
  enabled.value = !enabled.value
}

function startEditLabel() {
  editingLabel.value = true
  workLabelDraft.value = workLabel.value
}
function commitEditLabel() {
  const v = workLabelDraft.value.trim()
  // 清空则恢复为跟随界面语言的默认文案
  customWorkLabel.value = v
  editingLabel.value = false
  persistState()
}

function applyWorkMinutes() {
  let n = workMinutesInput.value
  if (!n || n < 1) n = 1
  workMinutesInput.value = n
  workDurationMs.value = n * 60 * 1000
  if (mode.value === 'work' && !isRunning.value) remainingMs.value = workDurationMs.value
  persistState()
}
function applyBreakMinutes() {
  let n = breakMinutesInput.value
  if (!n || n < 1) n = 1
  breakMinutesInput.value = n
  breakDurationMs.value = n * 60 * 1000
  if (mode.value === 'break' && !isRunning.value) remainingMs.value = breakDurationMs.value
  persistState()
}
function applyCycles() {
  let n = cyclesInput.value
  if (!n || n < 1) n = 1
  cyclesInput.value = n
  cyclesTotal.value = n
  if (cycleIndex.value > cyclesTotal.value) cycleIndex.value = cyclesTotal.value
  persistState()
}

function adjustWork(delta: number) {
  workMinutesInput.value += delta
  applyWorkMinutes()
}
function adjustBreak(delta: number) {
  breakMinutesInput.value += delta
  applyBreakMinutes()
}
function adjustCycles(delta: number) {
  cyclesInput.value += delta
  applyCycles()
}

// 折叠面板只隐藏界面，计时在后台继续（对应参考游戏的后台计时行为）：
// 重新展开时剩余时间由 tick 持续刷新，阶段切换的 AI 提醒也照常触发
watch(enabled, () => {
  persistState()
})

onMounted(() => {
  try {
    const savedEnabled = JSON.parse(localStorage.getItem(STORAGE_KEY_ENABLED) || 'false')
    const savedRemaining = JSON.parse(
      localStorage.getItem(STORAGE_KEY_REMAINING) || String(DEFAULT_WORK_MS),
    )
    const savedRunning = JSON.parse(localStorage.getItem(STORAGE_KEY_RUNNING) || 'false')
    const savedMode = (localStorage.getItem(STORAGE_KEY_MODE) as Mode) || 'work'
    const savedCycleIdx = JSON.parse(localStorage.getItem(STORAGE_KEY_CYCLE_INDEX) || '1')
    const savedCyclesTotal = JSON.parse(
      localStorage.getItem(STORAGE_KEY_CYCLES_TOTAL) || String(DEFAULT_CYCLES_TOTAL),
    )
    const savedWorkMs = JSON.parse(
      localStorage.getItem(STORAGE_KEY_WORK_MS) || String(DEFAULT_WORK_MS),
    )
    const savedBreakMs = JSON.parse(
      localStorage.getItem(STORAGE_KEY_BREAK_MS) || String(DEFAULT_BREAK_MS),
    )
    const savedWorkLabel = localStorage.getItem(STORAGE_KEY_WORK_LABEL) || ''
    const savedPhaseEndAt = JSON.parse(localStorage.getItem(STORAGE_KEY_PHASE_END_AT) || '0')
    const savedCompleted = JSON.parse(localStorage.getItem(STORAGE_KEY_COMPLETED) || 'false')

    enabled.value = !!savedEnabled
    workDurationMs.value = Number.isFinite(savedWorkMs) ? savedWorkMs : DEFAULT_WORK_MS
    breakDurationMs.value = Number.isFinite(savedBreakMs) ? savedBreakMs : DEFAULT_BREAK_MS
    cyclesTotal.value = Number.isFinite(savedCyclesTotal) ? savedCyclesTotal : DEFAULT_CYCLES_TOTAL
    cycleIndex.value = Number.isFinite(savedCycleIdx) ? savedCycleIdx : 1
    mode.value = savedMode === 'break' ? 'break' : 'work'
    remainingMs.value = Number.isFinite(savedRemaining) ? savedRemaining : workDurationMs.value
    // 旧版默认文案"工作"视为未自定义，迁移后跟随界面语言
    customWorkLabel.value = savedWorkLabel === '工作' ? '' : savedWorkLabel
    justCompleted.value = !!savedCompleted
    // 不再要求面板处于展开状态：折叠状态下退出，下次启动计时也在后台恢复
    isRunning.value = !!savedRunning && (savedPhaseEndAt > 0 || savedRemaining > 0)

    workMinutesInput.value = workDurationMs.value / 60000
    breakMinutesInput.value = breakDurationMs.value / 60000
    cyclesInput.value = cyclesTotal.value

    if (isRunning.value) {
      // 兼容旧版存档：没有保存结束时刻时用剩余时间折算出一个新的
      phaseEndAt.value = savedPhaseEndAt > 0 ? savedPhaseEndAt : Date.now() + remainingMs.value
      clearTimer()
      timerId = window.setInterval(tick, 1000)
      // 立即补跑页面关闭/挂起期间流逝的时间（含跨阶段推进）
      tick()
    }
  } catch {}
})

onUnmounted(() => {
  clearTimer()
  window.removeEventListener('pointermove', onScrubMove)
})
</script>

<style scoped>
/* Tailwind 默认不包含针对 input[type=number] 移除 spinners 的工具类。
  这里使用标准 CSS 确保在 Firefox 和 Webkit 内核浏览器中效果一致。
*/
.no-spin::-webkit-inner-spin-button,
.no-spin::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.no-spin {
  -moz-appearance: textfield;
  appearance: none;
}
</style>

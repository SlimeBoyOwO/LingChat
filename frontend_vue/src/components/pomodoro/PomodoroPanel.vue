<template>
  <div class="fixed top-5 left-5 z-[2000] flex flex-col gap-3 font-sans">
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
      <h3 class="text-lg font-bold m-0">番茄钟</h3>
    </Button>

    <Transition
      enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      enter-from-class="opacity-0 -translate-y-2"
      leave-to-class="opacity-0 -translate-y-2"
    >
      <div
        v-if="enabled"
        class="w-[260px] bg-[#12121c]/75 backdrop-blur-[20px] border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)] rounded-3xl p-6 text-white flex flex-col items-center box-border"
      >
        <div class="relative mb-6 outline-none">
          <div class="w-[180px] h-[180px] relative outline-none border-none">
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
              <circle class="fill-none stroke-white/10 stroke-[4]" cx="50" cy="50" r="45" />
              <circle
                class="fill-none stroke-[url(#gradient-ring)] stroke-[4] stroke-round transition-[stroke-dashoffset] duration-1000 ease-linear drop-shadow-[0_0_4px_rgba(79,172,254,0.5)]"
                cx="50"
                cy="50"
                r="45"
                :style="progressStyle"
              />
            </svg>

            <div class="absolute inset-0 flex flex-col items-center justify-center z-10">
              <div
                class="h-6 flex items-center justify-center mb-1 cursor-pointer group"
                @click="startEditLabel"
                title="点击修改名称"
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
                  class="w-[120px] bg-transparent border-0 border-b border-[#4facfe] text-white text-center text-base outline-none p-0 focus:ring-0"
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
                第 {{ cycleIndex }} / {{ cyclesTotal }} 轮
              </div>
            </div>
          </div>
        </div>

        <div class="flex items-center justify-between w-40 mb-6">
          <div
            class="w-12 h-12 rounded-full bg-white/10 flex items-center justify-center cursor-pointer transition-all duration-200 text-white hover:bg-white/20 hover:scale-105 active:scale-95"
            :class="{ 'opacity-30 pointer-events-none bg-transparent shadow-none': isRunning }"
            @click="start"
            title="开始"
          >
            <svg viewBox="0 0 24 24" width="28" height="28" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
          </div>

          <div
            class="w-12 h-12 rounded-full bg-white/10 flex items-center justify-center cursor-pointer transition-all duration-200 text-white hover:bg-white/20 hover:scale-105 active:scale-95"
            :class="{ 'opacity-30 pointer-events-none bg-transparent shadow-none': !isRunning }"
            @click="pause"
            title="暂停"
          >
            <svg viewBox="0 0 24 24" width="28" height="28" fill="currentColor">
              <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" />
            </svg>
          </div>

          <div
            class="w-12 h-12 rounded-full bg-white/10 flex items-center justify-center cursor-pointer transition-all duration-200 text-white hover:bg-white/20 hover:scale-105 active:scale-95"
            @click="reset"
            title="重置"
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
            <span class="text-[11px] text-white/50 mb-1.5">专注</span>
            <div class="flex items-center justify-center relative h-6">
              <input
                type="number"
                class="no-spin w-8 bg-transparent border-none text-white text-right font-medium text-[15px] outline-none p-0 appearance-none"
                v-model.number="workMinutesInput"
                @change="applyWorkMinutes"
              />
              <span class="text-[11px] text-white/50 pointer-events-none ml-0.5 mr-1">m</span>
              <div class="flex flex-col justify-center h-full gap-[2px]">
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
            <span class="text-[11px] text-white/50 mb-1.5">休息</span>
            <div class="flex items-center justify-center relative h-6">
              <input
                type="number"
                class="no-spin w-8 bg-transparent border-none text-white text-right font-medium text-[15px] outline-none p-0 appearance-none"
                v-model.number="breakMinutesInput"
                @change="applyBreakMinutes"
              />
              <span class="text-[11px] text-white/50 pointer-events-none ml-0.5 mr-1">m</span>
              <div class="flex flex-col justify-center h-full gap-[2px]">
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
            <span class="text-[11px] text-white/50 mb-1.5">循环</span>
            <div class="flex items-center justify-center relative h-6">
              <input
                type="number"
                class="no-spin w-8 bg-transparent border-none text-white text-right font-medium text-[15px] outline-none p-0 appearance-none"
                v-model.number="cyclesInput"
                @change="applyCycles"
              />
              <span class="text-[11px] text-white/50 pointer-events-none ml-0.5 mr-1">次</span>
              <div class="flex flex-col justify-center h-full gap-[2px]">
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
import { scriptHandler } from '../../api/websocket/handlers/script-handler'

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

type Mode = 'work' | 'break'

const DEFAULT_WORK_MS = 25 * 60 * 1000
const DEFAULT_BREAK_MS = 5 * 60 * 1000
const DEFAULT_CYCLES_TOTAL = 2

const enabled = ref(false)
const isRunning = ref(false)
const mode = ref<Mode>('work')
const workLabel = ref('工作')
const editingLabel = ref(false)
const workLabelDraft = ref('')

const workDurationMs = ref<number>(DEFAULT_WORK_MS)
const breakDurationMs = ref<number>(DEFAULT_BREAK_MS)
const cyclesTotal = ref<number>(DEFAULT_CYCLES_TOTAL)
const cycleIndex = ref<number>(1)

const remainingMs = ref<number>(DEFAULT_WORK_MS)
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
const progressStyle = computed(() => ({
  strokeDasharray: `${circumference}`,
  strokeDashoffset: `${(1 - progress.value) * circumference}`,
  transformOrigin: '50% 50%',
}))

const statusText = computed(() => {
  if (
    !isRunning.value &&
    remainingMs.value === currentTotalMs.value &&
    cycleIndex.value === 1 &&
    mode.value === 'work'
  ) {
    return '空闲中'
  }
  if (!isRunning.value) {
    return '空闲中'
  }
  return mode.value === 'work' ? '专注中' : '休息中'
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
  gameStore.addToDialogHistory({
    type: 'message',
    character: gameStore.avatar.user_name,
    content,
  })
  scriptHandler.sendMessage(content)
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

function persistState() {
  localStorage.setItem(STORAGE_KEY_ENABLED, JSON.stringify(enabled.value))
  localStorage.setItem(STORAGE_KEY_REMAINING, JSON.stringify(remainingMs.value))
  localStorage.setItem(STORAGE_KEY_RUNNING, JSON.stringify(isRunning.value))
  localStorage.setItem(STORAGE_KEY_MODE, mode.value)
  localStorage.setItem(STORAGE_KEY_CYCLE_INDEX, JSON.stringify(cycleIndex.value))
  localStorage.setItem(STORAGE_KEY_CYCLES_TOTAL, JSON.stringify(cyclesTotal.value))
  localStorage.setItem(STORAGE_KEY_WORK_MS, JSON.stringify(workDurationMs.value))
  localStorage.setItem(STORAGE_KEY_BREAK_MS, JSON.stringify(breakDurationMs.value))
  localStorage.setItem(STORAGE_KEY_WORK_LABEL, workLabel.value)
}

function clearTimer() {
  if (timerId) {
    clearInterval(timerId)
    timerId = null
  }
}

function tick() {
  const prevMode = mode.value
  const prevCycle = cycleIndex.value

  remainingMs.value = Math.max(0, remainingMs.value - 1000)
  if (remainingMs.value === 0) {
    if (mode.value === 'work') {
      mode.value = 'break'
      remainingMs.value = breakDurationMs.value
      sendUserPrompt(
        `{番茄钟提醒：第${prevCycle}/${cyclesTotal.value}轮专注结束，开始休息 ${formatMinutes(breakDurationMs.value)} 分钟。}`,
      )
    } else {
      if (cycleIndex.value < cyclesTotal.value) {
        cycleIndex.value += 1
        mode.value = 'work'
        remainingMs.value = workDurationMs.value
        sendUserPrompt(
          `{番茄钟提醒：休息结束，开始第${cycleIndex.value}/${cyclesTotal.value}轮专注（${workLabel.value}），时长 ${formatMinutes(workDurationMs.value)} 分钟}`,
        )
      } else {
        clearTimer()
        isRunning.value = false
        sendUserPrompt(
          `{番茄钟提醒：本次番茄钟已完成（专注 ${formatMinutes(workDurationMs.value)} 分钟 + 休息 ${formatMinutes(breakDurationMs.value)} 分钟 × ${cyclesTotal.value} 轮）。}`,
        )
      }
    }
  }
  persistState()
}

function start() {
  if (isRunning.value) return
  if (remainingMs.value <= 0) remainingMs.value = currentTotalMs.value
  isRunning.value = true
  clearTimer()
  timerId = window.setInterval(tick, 1000)
  persistState()

  const phaseText = mode.value === 'work' ? `开始专注（${workLabel.value}）` : '开始休息'
  sendUserPrompt(
    `{我启动了番茄钟：专注 ${formatMinutes(workDurationMs.value)} 分钟，休息 ${formatMinutes(breakDurationMs.value)} 分钟，共 ${cyclesTotal.value} 轮。现在${phaseText}，这是第${cycleIndex.value}/${cyclesTotal.value}轮。}`,
  )
}

function pause() {
  if (!isRunning.value) return
  isRunning.value = false
  clearTimer()
  persistState()
}

function reset() {
  mode.value = 'work'
  cycleIndex.value = 1
  remainingMs.value = workDurationMs.value
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
  workLabel.value = v || '工作'
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

watch(enabled, (val) => {
  if (!val) {
    clearTimer()
    isRunning.value = false
  }
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
    const savedWorkLabel = localStorage.getItem(STORAGE_KEY_WORK_LABEL) || '工作'

    enabled.value = !!savedEnabled
    workDurationMs.value = Number.isFinite(savedWorkMs) ? savedWorkMs : DEFAULT_WORK_MS
    breakDurationMs.value = Number.isFinite(savedBreakMs) ? savedBreakMs : DEFAULT_BREAK_MS
    cyclesTotal.value = Number.isFinite(savedCyclesTotal) ? savedCyclesTotal : DEFAULT_CYCLES_TOTAL
    cycleIndex.value = Number.isFinite(savedCycleIdx) ? savedCycleIdx : 1
    mode.value = savedMode === 'break' ? 'break' : 'work'
    remainingMs.value = Number.isFinite(savedRemaining) ? savedRemaining : workDurationMs.value
    workLabel.value = savedWorkLabel || '工作'
    isRunning.value = !!savedRunning && enabled.value && savedRemaining > 0

    workMinutesInput.value = workDurationMs.value / 60000
    breakMinutesInput.value = breakDurationMs.value / 60000
    cyclesInput.value = cyclesTotal.value

    if (isRunning.value) {
      clearTimer()
      timerId = window.setInterval(tick, 1000)
    }
  } catch {}
})

onUnmounted(() => {
  clearTimer()
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
}
</style>

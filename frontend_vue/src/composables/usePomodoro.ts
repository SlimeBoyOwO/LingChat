import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { scriptHandler } from '@/api/websocket/handlers/script-handler'

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

export function usePomodoro() {
  const gameStore = useGameStore()
  const uiStore = useUIStore()

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
    gameStore.appendGameMessage({
      type: 'message',
      displayName: gameStore.userName,
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
      cyclesTotal.value = Number.isFinite(savedCyclesTotal)
        ? savedCyclesTotal
        : DEFAULT_CYCLES_TOTAL
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
    // Don't clear timer here - let the component manage it
    // Only clear when explicitly stopped
  })

  return {
    // State
    enabled,
    isRunning,
    mode,
    workLabel,
    editingLabel,
    workLabelDraft,
    workDurationMs,
    breakDurationMs,
    cyclesTotal,
    cycleIndex,
    remainingMs,
    workMinutesInput,
    breakMinutesInput,
    cyclesInput,
    // Computed
    currentTotalMs,
    minutes,
    seconds,
    progress,
    progressStyle,
    statusText,
    // Methods
    start,
    pause,
    reset,
    toggleEnabled,
    startEditLabel,
    commitEditLabel,
    applyWorkMinutes,
    applyBreakMinutes,
    applyCycles,
    adjustWork,
    adjustBreak,
    adjustCycles,
  }
}

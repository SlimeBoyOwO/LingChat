<template>
  <div class="pomodoro-entry">
    <Button type="nav" :class="['entry-btn', { active: enabled }]" @click="toggleEnabled">
      <span class="entry-icon">🍅</span>
      <h3>番茄钟</h3>
    </Button>

    <Transition name="fade-slide">
      <div v-if="enabled" class="pomodoro-panel glass-effect">
        <!-- 上部：圆环与信息 -->
        <div class="ring-section">
          <div class="ring-wrap">
            <svg class="ring" viewBox="0 0 100 100">
              <defs>
                <linearGradient id="gradient-ring" x1="0%" y1="0%" x2="100%" y2="100%">
                  <stop offset="0%" stop-color="#4facfe" />
                  <stop offset="100%" stop-color="#00f2fe" />
                </linearGradient>
              </defs>
              <circle class="track" cx="50" cy="50" r="45" />
              <circle class="progress" cx="50" cy="50" r="45" :style="progressStyle" />
            </svg>

            <div class="ring-center">
              <div class="label-area" @click="startEditLabel" title="点击修改名称">
                <span v-if="!editingLabel" class="label-text">{{ workLabel }}</span>
                <input
                  v-else
                  v-model="workLabelDraft"
                  class="label-input"
                  @blur="commitEditLabel"
                  @keyup.enter="commitEditLabel"
                  autofocus
                />
              </div>

              <div class="time-display">{{ minutes }}:{{ seconds }}</div>

              <div class="status-text">{{ statusText }}</div>
              <div class="cycle-text">第 {{ cycleIndex }} / {{ cyclesTotal }} 轮</div>
            </div>
          </div>
        </div>

        <!-- 中部：纯图标控制栏 -->
        <div class="controls-bar">
          <div class="icon-btn play" :class="{ disabled: isRunning }" @click="start" title="开始">
            <svg viewBox="0 0 24 24" width="28" height="28" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
          </div>

          <div class="icon-btn pause" :class="{ disabled: !isRunning }" @click="pause" title="暂停">
            <svg viewBox="0 0 24 24" width="28" height="28" fill="currentColor">
              <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" />
            </svg>
          </div>

          <div class="icon-btn reset" @click="reset" title="重置">
            <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
              <path
                d="M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"
              />
            </svg>
          </div>
        </div>

        <!-- 底部：设置栏 (自定义箭头) -->
        <div class="settings-bar">
          <div class="setting-col">
            <span class="s-label">专注</span>
            <div class="s-input-wrap">
              <input
                type="number"
                class="no-spin"
                v-model.number="workMinutesInput"
                @change="applyWorkMinutes"
              />
              <span class="s-unit">m</span>
              <div class="custom-spinners">
                <div class="spin-btn up" @click="adjustWork(1)">
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 14l5-5 5 5z" />
                  </svg>
                </div>
                <div class="spin-btn down" @click="adjustWork(-1)">
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 10l5 5 5-5z" />
                  </svg>
                </div>
              </div>
            </div>
          </div>

          <div class="setting-col">
            <span class="s-label">休息</span>
            <div class="s-input-wrap">
              <input
                type="number"
                class="no-spin"
                v-model.number="breakMinutesInput"
                @change="applyBreakMinutes"
              />
              <span class="s-unit">m</span>
              <div class="custom-spinners">
                <div class="spin-btn up" @click="adjustBreak(1)">
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 14l5-5 5 5z" />
                  </svg>
                </div>
                <div class="spin-btn down" @click="adjustBreak(-1)">
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 10l5 5 5-5z" />
                  </svg>
                </div>
              </div>
            </div>
          </div>

          <div class="setting-col">
            <span class="s-label">循环</span>
            <div class="s-input-wrap">
              <input
                type="number"
                class="no-spin"
                v-model.number="cyclesInput"
                @change="applyCycles"
              />
              <span class="s-unit">次</span>
              <div class="custom-spinners">
                <div class="spin-btn up" @click="adjustCycles(1)">
                  <svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor">
                    <path d="M7 14l5-5 5 5z" />
                  </svg>
                </div>
                <div class="spin-btn down" @click="adjustCycles(-1)">
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
import { scriptHandler } from '../../api/websocket/handlers/script-handler'

const gameStore = useGameStore()

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
  transform: 'rotate(-90deg)',
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

/**
 * 以“正常聊天”的方式发送一条用户消息：
 * - 写入 dialogHistory（历史对话）
 * - 走 websocket 的 MESSAGE 通道发给后端（LLM）
 *
 * 为避免打断用户正在对话的流程：当不在 input 状态时先排队，等回到 input 再发。
 */
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

      // 休息开始：主动触发一次对话
      sendUserPrompt(
        `{番茄钟提醒：第${prevCycle}/${cyclesTotal.value}轮专注结束，开始休息 ${formatMinutes(breakDurationMs.value)} 分钟。}`,
      )
    } else {
      if (cycleIndex.value < cyclesTotal.value) {
        cycleIndex.value += 1
        mode.value = 'work'
        remainingMs.value = workDurationMs.value

        // 下一轮专注开始：主动触发一次对话
        sendUserPrompt(
          `{番茄钟提醒：休息结束，开始第${cycleIndex.value}/${cyclesTotal.value}轮专注（${workLabel.value}），时长 ${formatMinutes(workDurationMs.value)} 分钟}`,
        )
      } else {
        clearTimer()
        isRunning.value = false

        // 可选：循环全部完成时也触发一次（不强制）
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

  // 点击开始：主动触发一次对话（写入历史 + 发给后端）
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

// 核心逻辑：应用时间更改
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

// 辅助方法：点击自定义箭头时调用
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
/* 全局变量 */
.pomodoro-panel {
  --color-text: #ffffff;
  --color-text-dim: rgba(255, 255, 255, 0.5);
  --color-accent: #4facfe;
  --bg-glass: rgba(18, 18, 28, 0.75);
}

.pomodoro-entry {
  position: fixed;
  top: 20px;
  left: 20px;
  z-index: 2000;
  display: flex;
  flex-direction: column;
  gap: 12px;
  font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
}

.entry-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
}
.entry-icon {
  font-size: 1.2rem;
}

/* 主面板容器 */
.pomodoro-panel {
  width: 260px;
  background: var(--bg-glass);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  border-radius: 24px;
  padding: 24px 20px;
  color: var(--color-text);
  display: flex;
  flex-direction: column;
  align-items: center;
  box-sizing: border-box;
}

/* 1. 圆环区域 */
.ring-section {
  position: relative;
  margin-bottom: 24px;
}

.ring-wrap {
  width: 180px;
  height: 180px;
  position: relative;
  border: none !important;
  outline: none !important;
  box-shadow: none !important;
  background: transparent !important;
}

.ring {
  width: 100%;
  height: 100%;
  transform: rotate(-90deg);
  overflow: visible;
  border: none !important;
  outline: none !important;
  box-shadow: none !important;
  background: transparent !important;
}

/* 去掉部分浏览器/系统环境可能给 SVG 或可聚焦元素绘制的“直角白框” */
.ring-section,
.ring-section :deep(svg),
.ring-section :deep(svg:focus),
.ring-section :deep(svg:focus-visible),
.ring-section :deep(*:focus),
.ring-section :deep(*:focus-visible) {
  outline: none !important;
  box-shadow: none !important;
}

.track {
  fill: none;
  stroke: rgba(255, 255, 255, 0.1);
  stroke-width: 4;
}

.progress {
  fill: none;
  stroke: url(#gradient-ring);
  stroke-width: 4;
  stroke-linecap: round;
  transition: stroke-dashoffset 1s linear;
  filter: drop-shadow(0 0 4px rgba(79, 172, 254, 0.5));
}

.ring-center {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  z-index: 2;
}

/* 标签 */
.label-area {
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 4px;
  cursor: pointer;
}
.label-text {
  font-size: 16px;
  font-weight: 500;
  letter-spacing: 1px;
  text-align: center;
  opacity: 0.9;
}
.label-input {
  width: 120px;
  background: transparent;
  border: none;
  border-bottom: 1px solid var(--color-accent);
  color: #fff;
  text-align: center;
  font-size: 16px;
  outline: none;
}

/* 时间 */
.time-display {
  font-size: 48px;
  font-weight: 700;
  line-height: 1;
  font-variant-numeric: tabular-nums;
  margin: 4px 0;
  text-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

/* 状态 */
.status-text {
  font-size: 13px;
  color: var(--color-accent);
  font-weight: 600;
  margin-bottom: 2px;
}
.cycle-text {
  font-size: 11px;
  color: var(--color-text-dim);
}

/* 2. 控制栏 */
.controls-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 160px;
  margin-bottom: 24px;
}

.icon-btn {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.1);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s ease;
  color: #fff;
}
.icon-btn:hover {
  background: rgba(255, 255, 255, 0.2);
  transform: scale(1.05);
}
.icon-btn:active {
  transform: scale(0.95);
}
.icon-btn.disabled {
  opacity: 0.3;
  pointer-events: none;
  background: transparent;
  box-shadow: none;
}

/* 3. 底部设置栏 (改进版) */
.settings-bar {
  display: flex;
  justify-content: space-between;
  width: 100%;
  padding-top: 16px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.setting-col {
  display: flex;
  flex-direction: column;
  align-items: center;
  flex: 1;
}

.s-label {
  font-size: 11px;
  color: var(--color-text-dim);
  margin-bottom: 6px;
}

.s-input-wrap {
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  height: 24px;
}

.no-spin::-webkit-inner-spin-button,
.no-spin::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}
.no-spin {
  appearance: textfield;
  -moz-appearance: textfield;
  width: 32px;
  background: transparent;
  border: none;
  color: #fff;
  font-size: 15px;
  text-align: right;
  font-weight: 500;
  outline: none;
  padding: 0;
}

.s-unit {
  font-size: 11px;
  color: var(--color-text-dim);
  pointer-events: none;
  margin-left: 2px;
  margin-right: 4px;
}

/* 自定义微调箭头容器 */
.custom-spinners {
  display: flex;
  flex-direction: column;
  justify-content: center;
  height: 100%;
  gap: 2px;
}

.spin-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  opacity: 0.6;
  transition: opacity 0.2s;
  height: 10px;
}

.spin-btn:hover {
  opacity: 1;
}

.spin-btn:active {
  transform: scale(0.9);
}

/* 动画 */
.fade-slide-enter-active,
.fade-slide-leave-active {
  transition: all 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
}
.fade-slide-enter-from,
.fade-slide-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>

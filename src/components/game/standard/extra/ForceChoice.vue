<template>
  <!-- 强制选择（DDLC 式）：真实鼠标被磁力强行拖向指定选项，但点击必须玩家自己动手 -->
  <div
    v-if="gameStore.forceChoice"
    ref="overlayRef"
    class="force-choice-overlay"
    @mousemove="onMouseMove"
  >
    <div class="flex flex-col gap-10 w-full max-w-2xl px-4">
      <button
        v-for="choice in gameStore.forceChoice.choices"
        :key="choice.text"
        :disabled="choice.disabled || choice.text !== gameStore.forceChoice!.forced"
        :title="choice.disabled ? choice.reason || '该选项当前不可选' : ''"
        :class="[
          'relative w-full py-4 px-8 border rounded-full border-white/10 backdrop-blur-xl backdrop-saturate-150',
          choice.disabled && choice.text !== gameStore.forceChoice!.forced
            ? 'text-white/30 bg-slate-900/20'
            : choice.text === gameStore.forceChoice!.forced
              ? 'text-white bg-slate-900/40 forced-target'
              : 'text-white/40 bg-slate-900/20',
        ]"
        @click="onChoiceClick(choice)"
      >
        <span class="text-lg font-medium tracking-widest text-center block drop-shadow-[0_2px_4px_rgba(0,0,0,0.8)]">
          {{ choice.text }}
        </span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useGameStore } from '@/stores/modules/game'
import type { ScriptChoiceItem } from '@/types/script'

const gameStore = useGameStore()

const overlayRef = ref<HTMLElement | null>(null)

// 当前真实鼠标位置（由 mousemove 追踪；拖动期间会被我们不断改写）
const realPos = { x: window.innerWidth / 2, y: window.innerHeight / 2 }

let timerId = 0
let startedAt = 0
let submitted = false
let warpFailures = 0

/**
 * 按"选项在 choices 里的索引"从容器里取强制目标按钮。
 * 不用模板 :ref 条件绑定——函数 ref 在 v-for 重渲染下的回调时序不可靠，
 * 曾经出现 ref 指到错误按钮、鼠标被拖向相反选项的问题。
 */
function forcedBtn(): HTMLElement | null {
  const fc = gameStore.forceChoice
  const root = overlayRef.value
  if (!fc || !root) return null
  const idx = fc.choices.findIndex((c) => c.text === fc.forced)
  if (idx < 0) return null
  const buttons = root.querySelectorAll('button')
  return (buttons.item(idx) as HTMLElement) ?? null
}

function onMouseMove(e: MouseEvent) {
  // 玩家挣扎时取最新位置作为下一次拖拽的起点
  realPos.x = e.clientX
  realPos.y = e.clientY
}

const TICK_MS = 33 // 每帧拖动一步，约 30 步/秒
const PULL_MS = 2600 // 2.6s 后完全被吸附
const MAX_WARP_FAILURES = 5 // 连续失败这么多次就放弃拖动，留在原地等玩家自己点

async function tick() {
  const fc = gameStore.forceChoice
  if (!fc || submitted) return
  const btn = forcedBtn()
  if (!btn) {
    // 按钮尚未渲染完成：重试而不是静默退出，避免拖动完全不发生
    timerId = window.setTimeout(tick, TICK_MS)
    return
  }

  const elapsed = performance.now() - startedAt
  // 磁力曲线：前 0.4s 几乎正常，之后加速增强直至完全吸附
  const pull = Math.max(0, Math.min(1, (elapsed - 400) / (PULL_MS - 400)))

  const rect = btn.getBoundingClientRect()
  const tx = rect.left + rect.width / 2
  const ty = rect.top + rect.height / 2

  // 朝目标插值一步（步长随磁力变大），然后改写真实鼠标位置
  const step = 0.06 + pull * 0.3
  realPos.x += (tx - realPos.x) * step
  realPos.y += (ty - realPos.y) * step

  try {
    await invoke('warp_cursor', { x: realPos.x, y: realPos.y })
    warpFailures = 0
  } catch (e) {
    warpFailures += 1
    console.warn(`[ForceChoice] warp_cursor 失败(${warpFailures}/${MAX_WARP_FAILURES}):`, e)
    if (warpFailures >= MAX_WARP_FAILURES) {
      // 拖不动就放弃拖动、保持选项开着等玩家自己点——只有强制项可点，不会死锁
      console.warn('[ForceChoice] warp_cursor 持续失败，退化为普通点击选择')
      return
    }
    timerId = window.setTimeout(tick, TICK_MS)
    return
  }

  const dist = Math.hypot(realPos.x - tx, realPos.y - ty)
  if (pull >= 1 && dist < 4) {
    // 完全吸附后把指针钉在按钮中心，但【不替玩家点击】——继续拖动循环，
    // 玩家挣扎会被立刻拉回，直到玩家自己点下强制项（onChoiceClick）为止
    realPos.x = tx
    realPos.y = ty
    invoke('warp_cursor', { x: tx, y: ty }).catch(() => {})
  }

  timerId = window.setTimeout(tick, TICK_MS)
}

/** warp 不可用/配置错误时的兜底：直接自动提交 forced，避免剧本卡死（仅用于 forced 配置无效的异常情况） */
function finishFallback() {
  const fc = gameStore.forceChoice
  if (!fc || submitted) return
  submitted = true
  window.setTimeout(() => submit(fc.forced), 800)
}

/** 玩家自己点击：只有未被禁用的强制项会真正提交（其余按钮本就 disabled） */
function onChoiceClick(choice: ScriptChoiceItem) {
  const fc = gameStore.forceChoice
  if (!fc || submitted) return
  if (choice.disabled || choice.text !== fc.forced) return
  submitted = true
  submit(choice.text)
}

function submit(choice: string) {
  gameStore.appendGameMessage({
    type: 'message',
    displayName: gameStore.userName,
    content: choice,
  })
  invoke('script_submit_choice', { choice })
  gameStore.forceChoice = null
}

watch(
  () => gameStore.forceChoice,
  async (fc) => {
    clearTimeout(timerId)
    submitted = false
    warpFailures = 0
    if (!fc) return
    if (!fc.forced || !fc.choices.some((c) => c.text === fc.forced && !c.disabled)) {
      // forced 配置无效（剧本 bug，正常流程不会走到）：兜底自动提交 forced 原文，避免死锁
      finishFallback()
      return
    }
    await nextTick()
    startedAt = performance.now()
    timerId = window.setTimeout(tick, TICK_MS)
  },
)

onBeforeUnmount(() => clearTimeout(timerId))
</script>

<style scoped>
.force-choice-overlay {
  position: fixed;
  inset: 0;
  z-index: 900000;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  margin-top: -15vh;
  /* 父容器 GameExtraUI 是 pointer-events:none，必须显式夺回事件 */
  pointer-events: auto;
}

/* 非强制项在演出期间不可点 */
.force-choice-overlay button:disabled {
  cursor: not-allowed;
}

/* 被吸附的目标按钮：血色呼吸微光，像有什么在"推荐"它 */
.forced-target {
  animation: forced-breathe 1.4s ease-in-out infinite;
}

@keyframes forced-breathe {
  0%,
  100% {
    box-shadow: 0 0 8px rgba(184, 9, 26, 0.25);
    border-color: rgba(184, 9, 26, 0.35);
  }
  50% {
    box-shadow: 0 0 22px rgba(184, 9, 26, 0.55);
    border-color: rgba(184, 9, 26, 0.8);
  }
}
</style>

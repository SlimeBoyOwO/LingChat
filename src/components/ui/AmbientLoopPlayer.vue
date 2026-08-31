<template>
  <!-- 双 <audio> 实例：关闭原生 loop，在循环尾部用等功率交叉淡入淡出重叠播放，消除循环点击 -->
  <audio
    ref="audioA"
    @ended="handleEnded('A')"
    @timeupdate="handleTimeUpdate('A')"
  ></audio>
  <audio
    ref="audioB"
    @ended="handleEnded('B')"
    @timeupdate="handleTimeUpdate('B')"
  ></audio>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue'

const props = withDefaults(
  defineProps<{
    src: string
    volume: number // 0-1 有效音量（单轨音量 * 全局环境音音量）
    loop?: boolean
    fade?: boolean // 入场/出场是否启用淡入淡出
    paused?: boolean
    stopped?: boolean // true 时淡出后通知父级移除
  }>(),
  {
    loop: true,
    fade: true,
    paused: false,
    stopped: false,
  },
)

const emit = defineEmits<{
  (e: 'stopped-done'): void
}>()

const audioA = ref<HTMLAudioElement | null>(null)
const audioB = ref<HTMLAudioElement | null>(null)

// 淡入淡出时长
const ENTRY_FADE_MS = 1000
const EXIT_FADE_MS = 1000
const LOOP_CROSSFADE_MS = 2000 // 循环交叉淡入淡出略长于入场/出场，掩盖点击更稳

let fadeRaf: number | null = null
let crossRaf: number | null = null
const cancelFade = () => {
  if (fadeRaf !== null) {
    cancelAnimationFrame(fadeRaf)
    fadeRaf = null
  }
}
const cancelCross = () => {
  if (crossRaf !== null) {
    cancelAnimationFrame(crossRaf)
    crossRaf = null
  }
}

let activeKey: 'A' | 'B' = 'A'
let otherKey: 'A' | 'B' | null = null
let crossfading = false
let stopping = false
let started = false
let targetVolume = props.volume

const getEl = (k: 'A' | 'B'): HTMLAudioElement | null => (k === 'A' ? audioA.value : audioB.value)

const rampVolume = (
  el: HTMLAudioElement,
  from: number,
  to: number,
  ms: number,
  onDone?: () => void,
) => {
  cancelFade()
  const start = performance.now()
  const step = (now: number) => {
    const p = Math.min((now - start) / ms, 1)
    el.volume = Math.max(0, Math.min(1, from + (to - from) * p))
    if (p < 1) fadeRaf = requestAnimationFrame(step)
    else {
      fadeRaf = null
      onDone?.()
    }
  }
  fadeRaf = requestAnimationFrame(step)
}

const beginCrossfade = () => {
  if (crossfading || stopping || !props.loop) return
  const active = getEl(activeKey)
  const nextKey: 'A' | 'B' = activeKey === 'A' ? 'B' : 'A'
  const next = getEl(nextKey)
  if (!active || !next) return

  const dur = active.duration
  // 流式/未知时长交给 ended 兜底
  if (!isFinite(dur) || dur <= 0) return
  const cfMs = Math.min(LOOP_CROSSFADE_MS, (dur / 3) * 1000)
  if (cfMs <= 0) return

  next.src = props.src
  next.load()
  next.volume = 0
  next.play().catch(() => {})

  crossfading = true
  otherKey = nextKey
  const start = performance.now()
  const animate = (now: number) => {
    if (stopping) {
      crossfading = false
      otherKey = null
      crossRaf = null
      return
    }
    const p = Math.min((now - start) / cfMs, 1)
    // 等功率曲线：cos²+sin²=1，总功率恒定，对噪声类环境音比线性更自然
    const gOut = Math.cos((p * Math.PI) / 2)
    const gIn = Math.sin((p * Math.PI) / 2)
    active.volume = Math.max(0, Math.min(1, targetVolume * gOut))
    next.volume = Math.max(0, Math.min(1, targetVolume * gIn))
    if (p < 1) {
      crossRaf = requestAnimationFrame(animate)
    } else {
      active.pause()
      active.currentTime = 0
      active.volume = 0
      crossRaf = null
      crossfading = false
      otherKey = null
      activeKey = nextKey
    }
  }
  crossRaf = requestAnimationFrame(animate)
}

const handleTimeUpdate = (k: 'A' | 'B') => {
  if (k !== activeKey || crossfading || stopping || !props.loop) return
  const el = getEl(k)
  if (!el) return
  const dur = el.duration
  if (!isFinite(dur) || dur <= 0) return
  const cfSec = Math.min(LOOP_CROSSFADE_MS / 1000, dur / 3)
  if (el.currentTime >= dur - cfSec) beginCrossfade()
}

// 兜底：主路径未生效（如标签页被节流、timeupdate 暂停）时由 ended 触发
const handleEnded = (k: 'A' | 'B') => {
  if (k !== activeKey || crossfading || stopping) return
  if (props.loop) beginCrossfade()
}

const stopWithFade = () => {
  stopping = true
  cancelCross()
  crossfading = false
  otherKey = null
  const active = getEl(activeKey)
  if (!active) {
    emit('stopped-done')
    return
  }
  if (!props.fade) {
    active.volume = 0
    active.pause()
    emit('stopped-done')
    return
  }
  const from = active.volume
  rampVolume(active, from, 0, EXIT_FADE_MS, () => {
    getEl('A')?.pause()
    getEl('B')?.pause()
    emit('stopped-done')
  })
}

onMounted(() => {
  const active = getEl(activeKey)
  if (!active) return
  targetVolume = props.volume
  active.src = props.src
  active.load()
  if (props.fade) {
    active.volume = 0
  } else {
    active.volume = props.volume
  }
  if (!props.paused && !props.stopped) {
    active.play().catch((e) => console.warn('环境音播放失败:', e))
    if (props.fade) rampVolume(active, 0, props.volume, ENTRY_FADE_MS)
  }
  started = true
})

watch(
  () => props.src,
  (newSrc) => {
    if (!started || stopping) return
    cancelCross()
    crossfading = false
    otherKey = null
    const other = getEl(activeKey === 'A' ? 'B' : 'A')
    if (other) {
      other.pause()
      other.src = ''
    }
    const active = getEl(activeKey)
    if (!active) return
    targetVolume = props.volume
    active.src = newSrc
    active.load()
    if (props.fade) {
      active.volume = 0
      if (!props.paused && !props.stopped) {
        active.play().catch(() => {})
        rampVolume(active, 0, props.volume, ENTRY_FADE_MS)
      }
    } else {
      active.volume = props.volume
      if (!props.paused && !props.stopped) active.play().catch(() => {})
    }
  },
)

watch(
  () => props.volume,
  (v) => {
    targetVolume = v
    if (crossfading || stopping) return
    const active = getEl(activeKey)
    if (active) {
      cancelFade()
      active.volume = v
    }
  },
)

watch(
  () => props.paused,
  (isPaused) => {
    if (stopping) return
    const active = getEl(activeKey)
    const other = otherKey ? getEl(otherKey) : null
    if (isPaused) {
      active?.pause()
      other?.pause()
    } else {
      if (active && active.src) active.play().catch(() => {})
      if (crossfading && other && other.src) other.play().catch(() => {})
    }
  },
)

watch(
  () => props.stopped,
  (s) => {
    if (s && !stopping) stopWithFade()
  },
)

onBeforeUnmount(() => {
  cancelFade()
  cancelCross()
  const a = getEl('A')
  const b = getEl('B')
  if (a) {
    a.volume = 0
    a.pause()
    a.src = ''
  }
  if (b) {
    b.volume = 0
    b.pause()
    b.src = ''
  }
})
</script>

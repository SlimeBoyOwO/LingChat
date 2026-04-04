<template>
  <audio ref="audio1" @ended="handleEnded(1)"></audio>
  <audio ref="audio2" @ended="handleEnded(2)"></audio>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue'

const props = withDefaults(
  defineProps<{
    src?: string | null
    volume?: number // 0-100 音量
    paused?: boolean
    stopped?: boolean
    duration?: number // 淡入淡出时长 (毫秒)
  }>(),
  {
    src: '',
    volume: 100,
    paused: false,
    stopped: false,
    duration: 800,
  },
)

const emit = defineEmits<{
  (e: 'ended'): void
}>()

const audio1 = ref<HTMLAudioElement | null>(null)
const audio2 = ref<HTMLAudioElement | null>(null)

let activeIndex = 1 // 1 或 2，表示当前主音频
let fadeIntervalId: ReturnType<typeof setInterval> | null = null
const FADE_INTERVAL = 50

const clearFade = () => {
  if (fadeIntervalId !== null) {
    clearInterval(fadeIntervalId)
    fadeIntervalId = null
  }
}

onBeforeUnmount(() => {
  clearFade()
})

// 只派发当前主音频轨道的结束事件，忽略备用轨道的事件
const handleEnded = (index: number) => {
  if (index === activeIndex) {
    emit('ended')
  }
}

const crossFadeTo = async (newUrl: string | null | undefined) => {
  clearFade() // 立即停止前一次可能未完成的淡入淡出

  const currentAudio = activeIndex === 1 ? audio1.value : audio2.value
  const nextAudio = activeIndex === 1 ? audio2.value : audio1.value

  if (!currentAudio || !nextAudio) return

  const currentTargetVolume = props.volume / 100
  const step = currentTargetVolume / (props.duration / FADE_INTERVAL)

  // 1. 如果没有新的 URL (相当于仅仅是停止播放并淡出)
  if (!newUrl || newUrl === 'None') {
    fadeIntervalId = setInterval(() => {
      if (currentAudio.volume > 0) {
        currentAudio.volume = Math.max(0, currentAudio.volume - step)
      } else {
        currentAudio.pause()
        currentAudio.src = '' // 释放资源
        clearFade()
      }
    }, FADE_INTERVAL)
    return
  }

  // 2. 交叉淡入淡出新 URL
  nextAudio.src = newUrl
  nextAudio.load()
  nextAudio.volume = Math.min(0.1, props.volume / 100)

  // 只要没有被手动暂停或停止，就尝试播放备用轨道
  if (!props.paused && !props.stopped) {
    try {
      await nextAudio.play()
    } catch (err) {
      console.warn('由于浏览器策略，背景音乐自动播放被拦截:', err)
      // 即使被拦截，也继续走淡入逻辑，等到有了用户交互它就会自动出声
    }
  }

  fadeIntervalId = setInterval(() => {
    let currentDone = false
    let nextDone = false
    const targetVol = props.volume / 100
    // 动态步长，应对用户在渐变过程中拖动音量条
    const dynStep = targetVol / (props.duration / FADE_INTERVAL)

    // 旧音乐淡出
    if (currentAudio.volume > 0) {
      currentAudio.volume = Math.max(0, currentAudio.volume - dynStep)
    } else {
      currentDone = true
    }

    if (currentDone) {
      // 新音乐淡入
      if (nextAudio.volume < targetVol) {
        nextAudio.volume = Math.min(targetVol, nextAudio.volume + dynStep)
      } else {
        nextDone = true
      }
    }

    // 完成交接
    if (currentDone && nextDone) {
      currentAudio.pause()
      activeIndex = activeIndex === 1 ? 2 : 1 // 切换主轨道身份
      clearFade()
    }
  }, FADE_INTERVAL)
}

// 初始化
onMounted(() => {
  if (props.src && props.src !== 'None' && audio1.value) {
    audio1.value.src = props.src
    audio1.value.volume = props.volume / 100
    if (!props.paused && !props.stopped) {
      audio1.value.play().catch((e) => console.warn('初始化播放失败:', e))
    }
  }
})

// 监听 URL 变化触发交叉淡入淡出
watch(
  () => props.src,
  (newUrl) => {
    crossFadeTo(newUrl)
  },
)

// 监听音量变化
watch(
  () => props.volume,
  (newVol) => {
    if (fadeIntervalId === null) {
      const activeAudio = activeIndex === 1 ? audio1.value : audio2.value
      if (activeAudio) activeAudio.volume = newVol / 100
    }
  },
)

// 监听暂停
watch(
  () => props.paused,
  (isPaused) => {
    const activeAudio = activeIndex === 1 ? audio1.value : audio2.value
    if (!activeAudio) return
    if (isPaused) {
      activeAudio.pause()
    } else if (!props.stopped && activeAudio.src) {
      activeAudio.play().catch(() => {})
    }
  },
)

// 监听停止
watch(
  () => props.stopped,
  (isStopped) => {
    const activeAudio = activeIndex === 1 ? audio1.value : audio2.value
    if (!activeAudio) return
    if (isStopped) {
      activeAudio.pause()
      activeAudio.currentTime = 0
    } else if (!props.paused && activeAudio.src) {
      activeAudio.play().catch(() => {})
    }
  },
)
</script>

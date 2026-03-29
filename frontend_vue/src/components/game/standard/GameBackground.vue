<template>
  <div class="game-background" :style="backgroundStyle">
    <StarField
      ref="starfieldRef"
      v-if="uiStore.currentBackgroundEffect === `StarField`"
      :enabled="starfieldEnabled"
      :star-count="starCount"
      :scroll-speed="scrollSpeed"
      :colors="starColors"
      @ready="onStarfieldReady"
    />
    <Rain
      v-if="uiStore.currentBackgroundEffect === `Rain`"
      :enabled="rainEnabled"
      :intensity="rainIntensity"
    />
    <Sakura v-if="uiStore.currentBackgroundEffect === `Sakura`" :enabled="true" :intensity="1.5">
    </Sakura>
    <Snow
      v-if="uiStore.currentBackgroundEffect === `Snow`"
      :intensity="snowIntensity"
      :enabled="true"
    >
    </Snow>
    <Fireworks
      v-if="uiStore.currentBackgroundEffect === `Fireworks`"
      :enabled="true"
      :intensity="1.5"
    />
  </div>
  <audio ref="soundEffectPlayer"></audio>
  <audio ref="backgroundMusicPlayer" @ended="handleTrackEnd"></audio>
</template>

<script setup lang="ts">
import { ref, computed, watch, type CSSProperties } from 'vue'
import { useUIStore } from '../../../stores/modules/ui/ui'
import StarField from './particles/StarField.vue'
import Rain from './particles/Rain.vue'
import Sakura from './particles/Sakura.vue'
import Snow from './particles/Snow.vue'
import Fireworks from './particles/Fireworks.vue'

const uiStore = useUIStore()

// 明确指定 DOM ref 为 HTMLAudioElement 类型，初始值为 null
const soundEffectPlayer = ref<HTMLAudioElement | null>(null)
const backgroundMusicPlayer = ref<HTMLAudioElement | null>(null)

const FADE_DURATION: number = 800 // 淡入/淡出各持续时间 (毫秒)
const FADE_INTERVAL: number = 50 // 每次音量变化的间隔 (毫秒)

// 兼容浏览器(number)和Node/Vite环境(NodeJS.Timeout)的定时器类型
let fadeTimer: ReturnType<typeof setInterval> | null = null

// 星空效果控制
const starfieldEnabled = ref<boolean>(true)
const starCount = ref<number>(200)
const scrollSpeed = ref<number>(0.4)
const starColors = ref<string[]>([
  'rgb(173, 216, 230)',
  'rgb(176, 224, 230)',
  'rgb(241, 141, 252)',
  'rgb(176, 230, 224)',
  'rgb(173, 230, 216)',
])

// 雨滴效果控制
const rainEnabled = ref<boolean>(true)
const rainIntensity = ref<number>(1)

const snowIntensity = ref<number>(1.5)

// 计算背景样式，使用 Vue 内置的 CSSProperties 类型
const backgroundStyle = computed<CSSProperties>(() => {
  return {
    backgroundImage: uiStore.currentBackground
      ? `url(${uiStore.currentBackground})`
      : 'url(@/assets/images/default_bg.jpg)',
  }
})

const handleTrackEnd = (): void => {
  // 调用store的action处理背景音乐结束事件
  uiStore.handleBackgroundMusicEnd()
}

// 星空就绪回调 (如果知道 Starfield 组件的实例类型，可替换 any)
const onStarfieldReady = (instance: any): void => {
  console.debug('Starfield ready', instance)
}

// 核心：带有淡入淡出效果的音乐切换函数
const switchBackgroundMusic = (
  player: HTMLAudioElement,
  newUrl: string | null | undefined,
): void => {
  // 清除之前可能正在进行的淡入淡出操作
  if (fadeTimer !== null) {
    clearInterval(fadeTimer)
    fadeTimer = null
  }

  // 计算每一步音量变化的幅度
  const step: number = uiStore.backgroundVolume / 100 / (FADE_DURATION / FADE_INTERVAL)

  // --- 阶段1: 淡出 (Fade Out) ---
  const fadeOut = (): Promise<void> => {
    return new Promise((resolve) => {
      // 如果当前没有在播放，或者音量已经是0，直接完成
      if (player.paused || player.volume <= 0) {
        player.volume = 0
        resolve()
        return
      }

      fadeTimer = setInterval(() => {
        if (player.volume > 0) {
          // 确保音量不会减成负数
          player.volume = Math.max(0, player.volume - step)
        } else {
          // 淡出完成
          if (fadeTimer !== null) {
            clearInterval(fadeTimer)
            fadeTimer = null
          }
          player.pause() // 暂停旧音乐
          resolve()
        }
      }, FADE_INTERVAL)
    })
  }

  // --- 阶段2: 切换并淡入 (Switch & Fade In) ---
  const loadAndFadeIn = (): void => {
    if (!newUrl) return // TS安全校验

    // 设置新源
    player.src = newUrl
    player.load()
    player.volume = 0 // 确保开始时静音

    // 尝试播放
    const playPromise = player.play()

    if (playPromise !== undefined) {
      playPromise
        .then(() => {
          // 播放成功，开始淡入
          fadeTimer = setInterval(() => {
            const targetVolume = uiStore.backgroundVolume / 100
            if (player.volume < targetVolume) {
              // 确保音量不会超过目标值
              player.volume = Math.min(targetVolume, player.volume + step)
            } else {
              // 淡入完成
              if (fadeTimer !== null) {
                clearInterval(fadeTimer)
                fadeTimer = null
              }
            }
          }, FADE_INTERVAL)
        })
        .catch((error: Error | unknown) => {
          console.error('背景音乐自动播放失败:', error)
        })
    }
  }

  // 执行流程：先淡出 -> 再加载并淡入
  fadeOut().then(() => {
    // 如果新URL是 None 或空，淡出后就不再播放了
    if (!newUrl || newUrl === 'None') {
      player.src = ''
      return
    }
    loadAndFadeIn()
  })
}

// 监听音效
watch(
  () => uiStore.currentSoundEffect,
  (newAudioUrl: string | null | undefined) => {
    if (soundEffectPlayer.value && newAudioUrl && newAudioUrl !== 'None') {
      soundEffectPlayer.value.src = newAudioUrl
      soundEffectPlayer.value.load()
      soundEffectPlayer.value.play()
    }
  },
)

// 监听背景音乐
watch(
  () => uiStore.currentBackgroundMusic,
  (newAudioUrl: string | null | undefined) => {
    console.log('触发了新的背景音乐：newAudioUrl: ', newAudioUrl)

    if (backgroundMusicPlayer.value) {
      // 如果没有新音乐或为None，也执行平滑淡出停止
      uiStore.bgMusicPaused = false
      uiStore.bgMusicStoped = false
      switchBackgroundMusic(backgroundMusicPlayer.value, newAudioUrl)
    }
  },
)

// 监听音量
watch(
  () => uiStore.backgroundVolume,
  (newVolume: number) => {
    if (backgroundMusicPlayer.value) {
      backgroundMusicPlayer.value.volume = newVolume / 100
    }
  },
)

// 监听音乐暂停状态
watch(
  () => uiStore.bgMusicPaused,
  (isPaused: boolean) => {
    if (backgroundMusicPlayer.value) {
      if (isPaused) {
        backgroundMusicPlayer.value.pause()
      } else if (backgroundMusicPlayer.value.paused) {
        backgroundMusicPlayer.value.play()
      }
    }
  },
)

// 监听音乐停止状态
watch(
  () => uiStore.bgMusicStoped,
  (isStopped: boolean) => {
    if (backgroundMusicPlayer.value && isStopped) {
      backgroundMusicPlayer.value.pause()
      backgroundMusicPlayer.value.currentTime = 0
    }
  },
)
</script>

<style scoped>
.game-background {
  position: absolute;
  width: 100%;
  height: 100%;
  background-size: cover;
  background-position: center center;
  background-attachment: fixed;
  background-repeat: no-repeat;
  z-index: -2;
  transition: background-image 0.5s ease-in-out;
}
</style>

<template>
  <!-- 背景图，已使用 Tailwind 类替代原本的 css -->
  <ImageAcrossFade
    ref="imageFadeRef"
    class="game-background"
    :src="uiStore.currentBackground || '@/assets/images/default_bg.webp'"
    position="center center"
    object-fit="cover"
    :duration="uiStore.currentBackgroundTransition"
  >
    <StarField
      ref="starfieldRef"
      v-if="uiStore.currentBackgroundEffect === 'StarField'"
      :enabled="starfieldEnabled"
      :star-count="starCount"
      :scroll-speed="scrollSpeed"
      :colors="starColors"
      :style="`z-index:${BACKGROUND_ZINDEX}`"
      @ready="onStarfieldReady"
    />
    <Rain
      v-if="uiStore.currentBackgroundEffect === 'Rain'"
      :enabled="rainEnabled"
      :intensity="rainIntensity"
      :style="`z-index:${BACKGROUND_ZINDEX}`"
    />
    <Sakura
      v-if="uiStore.currentBackgroundEffect === 'Sakura'"
      :enabled="true"
      :intensity="1.5"
      :style="`z-index:${BACKGROUND_ZINDEX}`"
    />
    <Snow
      v-if="uiStore.currentBackgroundEffect === 'Snow'"
      :intensity="snowIntensity"
      :enabled="true"
      :style="`z-index:${BACKGROUND_ZINDEX}`"
    />
    <Fireworks
      v-if="uiStore.currentBackgroundEffect === 'Fireworks'"
      :enabled="true"
      :intensity="1.5"
      :style="`z-index:${BACKGROUND_ZINDEX}`"
    />
  </ImageAcrossFade>

  <!-- 短效音效保留默认实现即可，不需要淡入淡出 -->
  <audio ref="soundEffectPlayer"></audio>

  <!-- 全新解耦出来的双轨交叉音乐淡入淡出组件 -->
  <AudioAcrossFade
    :src="uiStore.currentBackgroundMusic"
    :volume="uiStore.backgroundVolume"
    :paused="uiStore.bgMusicPaused"
    :stopped="uiStore.bgMusicStoped"
    :duration="800"
    :loop="uiStore.bgMusicMode === 'loop-single'"
    @ended="handleTrackEnd"
  />

  <!-- 环境音多轨渲染 -->
  <audio
    v-for="track in uiStore.ambientTracks"
    :key="track.id"
    :ref="(el: any) => setAmbientRef(track.id, el as HTMLAudioElement)"
    :loop="track.loop"
  ></audio>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useUIStore } from '../../../stores/modules/ui/ui'
import type { AmbientTrack } from '../../../stores/modules/ui/ui'
import ImageAcrossFade from '@/components/ui/ImageAcrossFade.vue'
import AudioAcrossFade from '@/components/ui/AudioAcrossFade.vue' // 引入组件
import StarField from './particles/StarField.vue'
import Rain from './particles/Rain.vue'
import Sakura from './particles/Sakura.vue'
import Snow from './particles/Snow.vue'
import Fireworks from './particles/Fireworks.vue'

const uiStore = useUIStore()

// 背景效果 z-index 应该比其他组件高，否则会被覆盖
const BACKGROUND_ZINDEX = 114514

// 仅保留不需要淡入淡出的短效音效
const soundEffectPlayer = ref<HTMLAudioElement | null>(null)

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

// 其他特效参数控制
const rainEnabled = ref<boolean>(true)

const rainIntensity = ref<number>(1)
const snowIntensity = ref<number>(1.5)

// ====== 环境音多轨管理 ======
const ambientRefs = new Map<string, HTMLAudioElement>()
const fadeTimers = new Map<string, number>()

function setAmbientRef(id: string, el: HTMLAudioElement | null) {
  if (el) {
    ambientRefs.set(id, el)
  } else {
    ambientRefs.delete(id)
  }
}

function fadeInAmbient(
  audioEl: HTMLAudioElement,
  trackId: string,
  targetVolume: number,
  durationMs = 1000,
) {
  // 清除旧的淡入定时器
  const oldTimer = fadeTimers.get(trackId)
  if (oldTimer) {
    cancelAnimationFrame(oldTimer)
  }

  const startTime = performance.now()
  const startVolume = audioEl.volume

  function step(currentTime: number) {
    const elapsed = currentTime - startTime
    const progress = Math.min(elapsed / durationMs, 1)
    const eased = 1 - Math.pow(1 - progress, 2) // ease-out quad
    audioEl.volume = startVolume + (targetVolume - startVolume) * eased

    if (progress < 1) {
      fadeTimers.set(trackId, requestAnimationFrame(step))
    } else {
      audioEl.volume = targetVolume
      fadeTimers.delete(trackId)
    }
  }

  fadeTimers.set(trackId, requestAnimationFrame(step))
}

function fadeOutAmbient(audioEl: HTMLAudioElement, trackId: string, durationMs = 1000) {
  // 清除旧的淡入淡出定时器
  const oldTimer = fadeTimers.get(trackId)
  if (oldTimer) {
    cancelAnimationFrame(oldTimer)
  }

  const startTime = performance.now()
  const startVolume = audioEl.volume

  function step(currentTime: number) {
    const elapsed = currentTime - startTime
    const progress = Math.min(elapsed / durationMs, 1)
    const eased = 1 - Math.pow(1 - progress, 2) // ease-out quad
    audioEl.volume = startVolume * (1 - eased)

    if (progress < 1) {
      fadeTimers.set(trackId, requestAnimationFrame(step))
    } else {
      audioEl.volume = 0
      audioEl.pause()
      audioEl.src = ''
      fadeTimers.delete(trackId)
    }
  }

  fadeTimers.set(trackId, requestAnimationFrame(step))
}

function getAmbientHttpUrl(src: string): string {
  // 如果已经是完整 URL 或相对路径以 / 开头，直接返回
  if (src.startsWith('http') || src.startsWith('/')) return src
  // 否则视为文件名，拼接脚本环境音文件服务
  return `/api/v1/chat/script/ambient_file/${encodeURIComponent(src)}`
}

// 监听环境音轨道变化
watch(
  () => uiStore.ambientTracks,
  (newTracks: AmbientTrack[], oldTracks: AmbientTrack[] | undefined) => {
    const ambientVolume = uiStore.ambientVolume / 100

    // 处理新增轨道
    for (const track of newTracks) {
      const oldTrack = oldTracks?.find((t) => t.id === track.id)
      if (oldTrack) continue // 已存在，跳过

      const audioEl = ambientRefs.get(track.id)
      if (!audioEl) continue

      const url = getAmbientHttpUrl(track.src)
      audioEl.src = url
      audioEl.loop = track.loop
      audioEl.volume = 0 // 从静音开始淡入
      audioEl.play().catch(() => {})

      if (track.fade) {
        const targetVol = (track.volume / 100) * ambientVolume
        fadeInAmbient(audioEl, track.id, targetVol)
      } else {
        audioEl.volume = (track.volume / 100) * ambientVolume
      }
    }

    // 处理已移除轨道
    if (oldTracks) {
      for (const oldTrack of oldTracks) {
        const stillExists = newTracks.some((t) => t.id === oldTrack.id)
        if (stillExists) continue

        const audioEl = ambientRefs.get(oldTrack.id)
        if (!audioEl) continue

        if (oldTrack.fade) {
          fadeOutAmbient(audioEl, oldTrack.id, 800)
        } else {
          audioEl.pause()
          audioEl.src = ''
        }
      }
    }
  },
  { deep: true },
)

// 监听环境音全局音量变化
watch(
  () => uiStore.ambientVolume,
  () => {
    const ambientVolume = uiStore.ambientVolume / 100
    for (const track of uiStore.ambientTracks) {
      const audioEl = ambientRefs.get(track.id)
      if (audioEl) {
        audioEl.volume = (track.volume / 100) * ambientVolume
      }
    }
  },
)

// 监听环境音轨道暂停/恢复
watch(
  () => uiStore.ambientTracks.map((t) => t.paused),
  (newPausedStates, oldPausedStates) => {
    uiStore.ambientTracks.forEach((track, index) => {
      const audioEl = ambientRefs.get(track.id)
      if (!audioEl) return

      const wasPaused = oldPausedStates?.[index]
      const isNowPaused = newPausedStates[index]

      if (wasPaused !== isNowPaused) {
        if (isNowPaused) {
          if (track.fade) {
            fadeOutAmbient(audioEl, track.id, 400)
          } else {
            audioEl.pause()
          }
        } else if (!isNowPaused) {
          const ambientVolume = uiStore.ambientVolume / 100
          const targetVol = (track.volume / 100) * ambientVolume
          audioEl.volume = 0
          audioEl.play().catch(() => {})
          if (track.fade) {
            fadeInAmbient(audioEl, track.id, targetVol, 400)
          } else {
            audioEl.volume = targetVol
          }
        }
      }
    })
  },
  { deep: true },
)

// ====== BGM 相关 ======

const handleTrackEnd = (): void => {
  uiStore.handleBackgroundMusicEnd()
}

// 星空就绪回调
const onStarfieldReady = (_instance: any): void => {}

// 只保留监听瞬时音效 (由于音效很短，不需要淡入淡出，保持原生调用)
watch(
  () => uiStore.currentSoundEffect,
  (newAudioUrl: string | null | undefined) => {
    if (soundEffectPlayer.value && newAudioUrl && newAudioUrl !== 'None') {
      soundEffectPlayer.value.src = newAudioUrl
      soundEffectPlayer.value.load()
      soundEffectPlayer.value.play().catch(() => {})
    }
  },
)

// !!! 在此处：因为把背景音乐交给了 AudioCrossFade 组件，所以原先的大段背景音乐逻辑全被彻底删除。
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
}
</style>

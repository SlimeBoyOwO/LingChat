<template>
  <!-- 背景图 + 背景光照滤镜 -->
  <div class="absolute inset-0" :style="bgLightingFilter">
    <ImageAcrossFade
      ref="imageFadeRef"
      class="game-background"
      :src="backgroundSrc"
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
  </div>

  <!-- 背景光照叠加层（在背景上方、角色下方） -->
  <div
    v-if="bgOverlayStyle"
    class="absolute inset-0 pointer-events-none"
    :style="bgOverlayStyle as any"
  ></div>

  <!-- 短效音效保留默认实现即可，不需要淡入淡出 -->
  <audio ref="soundEffectPlayer"></audio>

  <!-- 全新解耦出来的双轨交叉音乐淡入淡出组件 -->
  <AudioAcrossFade
    :src="backgroundMusicSrc"
    :volume="uiStore.backgroundVolume"
    :paused="uiStore.bgMusicPaused"
    :stopped="uiStore.bgMusicStoped"
    :duration="800"
    :loop="uiStore.bgMusicMode === 'loop-single'"
    @ended="handleTrackEnd"
  />

  <!-- 环境音多轨渲染（每轨独立 <audio> 实例，最多8轨并行） -->
  <audio
    v-for="track in uiStore.ambientTracks"
    :key="track.id"
    :ref="(el: any) => setAmbientRef(track.id, el as HTMLAudioElement)"
    :loop="track.loop"
  ></audio>
</template>

<script setup lang="ts">
import { ref, watch, computed, nextTick } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useGameStore } from '../../../stores/modules/game'
import ImageAcrossFade from '@/components/ui/ImageAcrossFade.vue'
import AudioAcrossFade from '@/components/ui/AudioAcrossFade.vue'
import StarField from './particles/StarField.vue'
import Rain from './particles/Rain.vue'
import Sakura from './particles/Sakura.vue'
import Snow from './particles/Snow.vue'
import Fireworks from './particles/Fireworks.vue'

const uiStore = useUIStore()
const gameStore = useGameStore()

const backgroundSrc = computed(() => {
  const bg = uiStore.currentBackground
  if (
    !bg ||
    bg.startsWith('http://') ||
    bg.startsWith('https://') ||
    bg.startsWith('@/') ||
    bg.startsWith('data:')
  ) {
    return bg || '@/assets/images/default_bg.jpg'
  }
  return convertFileSrc(bg)
})

// 统一转换入口：currentBackgroundMusic 存储原始路径，在此一次性转换
const backgroundMusicSrc = computed(() => {
  const src = uiStore.currentBackgroundMusic
  if (!src || src === 'None') return 'None'
  return convertFileSrc(src)
})

// 背景光照滤镜
const bgLightingFilter = computed(() => {
  const c = gameStore.currentScene?.lighting?.background
  if (!c) return undefined
  const parts: string[] = []
  if (c.brightness !== 1.0) parts.push(`brightness(${c.brightness})`)
  if (c.contrast !== 1.0) parts.push(`contrast(${c.contrast})`)
  if (c.saturation !== 1.0) parts.push(`saturate(${c.saturation})`)
  if (c.glow_radius > 0) parts.push(`drop-shadow(0 0 ${c.glow_radius}px ${c.glow_color})`)
  if (c.sepia > 0) parts.push(`sepia(${c.sepia})`)
  return parts.length > 0 ? { filter: parts.join(' ') } : undefined
})

// 背景光照叠加层（仅当 target 为 background 或 both 时启用）
const bgOverlayStyle = computed(() => {
  const l = gameStore.currentScene?.lighting
  if (!l?.overlay_enabled) return undefined
  if (l.overlay_target !== 'background' && l.overlay_target !== 'both') return undefined
  const blend = l.blend_mode !== 'normal' ? l.blend_mode : 'overlay'
  return {
    background: `radial-gradient(circle at ${l.light_x}% ${l.light_y}%, ${l.overlay_color1} 0%, ${l.overlay_color2} ${l.overlay_radius}%)`,
    mixBlendMode: blend,
    opacity: l.overlay_opacity,
  }
})

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

const handleTrackEnd = (): void => {
  uiStore.handleBackgroundMusicEnd()
}

// 星空就绪回调
const onStarfieldReady = (instance: any): void => {
  console.debug('Starfield ready', instance)
}

// 只保留监听瞬时音效 (由于音效很短，不需要淡入淡出，保持原生调用)
watch(
  () => uiStore.currentSoundEffect,
  (newAudioUrl: string | null | undefined) => {
    if (soundEffectPlayer.value && newAudioUrl && newAudioUrl !== 'None') {
      // 重置 src 确保相同路径的重复事件也能触发播放
      soundEffectPlayer.value.pause()
      soundEffectPlayer.value.currentTime = 0
      soundEffectPlayer.value.src = ''
      soundEffectPlayer.value.src = newAudioUrl
      soundEffectPlayer.value.load()
      soundEffectPlayer.value.play()
    }
  },
)

// !!! 在此处：因为把背景音乐交给了 AudioCrossFade 组件，所以原先的大段背景音乐逻辑全被彻底删除。

// ========== 环境音多轨管理 ==========
// 存储各环境音轨道的 <audio> DOM 引用
const ambientRefs = ref<Map<string, HTMLAudioElement>>(new Map())

// 设置/清除环境音轨道的 DOM 引用
const setAmbientRef = (id: string, el: HTMLAudioElement | null) => {
  if (el) {
    ambientRefs.value.set(id, el)
  } else {
    ambientRefs.value.delete(id)
  }
}

// ========== 环境音淡入淡出 ==========
const FADE_DURATION = 1000  // 淡入淡出时长（毫秒）
const fadeTimers = ref<Map<string, number>>(new Map())

/** 淡入：从 0 渐增到目标音量 */
const fadeInAmbient = (audioEl: HTMLAudioElement, trackId: string, targetVolume: number) => {
  // 取消该轨道已有的淡入淡出
  const existing = fadeTimers.value.get(trackId)
  if (existing) cancelAnimationFrame(existing)

  audioEl.volume = 0
  const startTime = performance.now()

  const animate = (now: number) => {
    const elapsed = now - startTime
    const progress = Math.min(elapsed / FADE_DURATION, 1)
    audioEl.volume = progress * targetVolume

    if (progress < 1) {
      fadeTimers.value.set(trackId, requestAnimationFrame(animate))
    } else {
      fadeTimers.value.delete(trackId)
    }
  }
  fadeTimers.value.set(trackId, requestAnimationFrame(animate))
}

/** 淡出：从当前音量渐减到 0，完成后暂停 */
const fadeOutAmbient = (audioEl: HTMLAudioElement, trackId: string) => {
  const existing = fadeTimers.value.get(trackId)
  if (existing) cancelAnimationFrame(existing)

  const startVolume = audioEl.volume
  const startTime = performance.now()

  const animate = (now: number) => {
    const elapsed = now - startTime
    const progress = Math.min(elapsed / FADE_DURATION, 1)
    audioEl.volume = startVolume * (1 - progress)

    if (progress < 1) {
      fadeTimers.value.set(trackId, requestAnimationFrame(animate))
    } else {
      audioEl.pause()
      fadeTimers.value.delete(trackId)
    }
  }
  fadeTimers.value.set(trackId, requestAnimationFrame(animate))
}

// 监听环境音轨道列表变化，自动播放新增轨道、清理已移除轨道
watch(
  () => uiStore.ambientTracks,
  (newTracks, oldTracks) => {
    // 播放新增的轨道
    nextTick(() => {
      for (const track of newTracks) {
        const audioEl = ambientRefs.value.get(track.id)
        if (audioEl && !audioEl.src) {
          // blob URL 直接使用，文件系统路径需要转换
          audioEl.src = track.src.startsWith('blob:') ? track.src : convertFileSrc(track.src)
          const targetVolume = (track.volume / 100) * (uiStore.ambientVolume / 100)
          audioEl.loop = track.loop
          audioEl.load()
          audioEl.play().catch((e) => console.warn('环境音播放失败:', e))
          // 如果启用淡入，从 0 渐增到目标音量
          if (track.fade) {
            fadeInAmbient(audioEl, track.id, targetVolume)
          } else {
            audioEl.volume = targetVolume
          }
        }
      }
    })
    // 清理已移除的轨道（淡出后释放）
    const trackIds = new Set(newTracks.map(t => t.id))
    for (const [id, el] of ambientRefs.value.entries()) {
      if (!trackIds.has(id)) {
        // 检查被移除的轨道是否启用了淡出
        const removedTrack = oldTracks?.find(t => t.id === id)
        if (removedTrack?.fade) {
          fadeOutAmbient(el, id)
          // 淡出完成后清理（由 fadeOutAmbient 内部处理 pause）
          setTimeout(() => {
            el.src = ''
            ambientRefs.value.delete(id)
          }, FADE_DURATION + 100)
        } else {
          el.pause()
          el.src = ''
          ambientRefs.value.delete(id)
        }
      }
    }
  },
  { deep: true }
)

// 监听全局环境音音量变化，实时更新所有轨道音量
watch(
  () => uiStore.ambientVolume,
  (newVol) => {
    if (newVol == null) return // 防御：持久化数据可能缺少此字段
    for (const [id, el] of ambientRefs.value.entries()) {
      const track = uiStore.ambientTracks.find(t => t.id === id)
      if (track) {
        el.volume = (track.volume / 100) * (newVol / 100)
      }
    }
  }
)

// 监听单轨音量变化，实时更新对应 <audio> 元素音量
watch(
  () => uiStore.ambientTracks.map(t => `${t.id}:${t.volume}`).join(','),
  () => {
    for (const [id, el] of ambientRefs.value.entries()) {
      const track = uiStore.ambientTracks.find(t => t.id === id)
      if (track) {
        el.volume = (track.volume / 100) * (uiStore.ambientVolume / 100)
      }
    }
  }
)

// 监听单轨暂停状态变化
watch(
  () => uiStore.ambientTracks.map(t => `${t.id}:${t.paused}`).join(','),
  () => {
    for (const [id, el] of ambientRefs.value.entries()) {
      const track = uiStore.ambientTracks.find(t => t.id === id)
      if (track) {
        if (track.paused) {
          el.pause()
        } else if (el.paused && el.src) {
          el.play().catch((e) => console.warn('环境音恢复播放失败:', e))
        }
      }
    }
  }
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
}
</style>

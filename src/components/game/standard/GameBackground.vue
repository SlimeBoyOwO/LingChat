<template>
  <!-- 背景图，已使用 Tailwind 类替代原本的 css -->
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
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useUIStore } from '../../../stores/modules/ui/ui'
import ImageAcrossFade from '@/components/ui/ImageAcrossFade.vue'
import AudioAcrossFade from '@/components/ui/AudioAcrossFade.vue'
import StarField from './particles/StarField.vue'
import Rain from './particles/Rain.vue'
import Sakura from './particles/Sakura.vue'
import Snow from './particles/Snow.vue'
import Fireworks from './particles/Fireworks.vue'

const uiStore = useUIStore()

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

const backgroundMusicSrc = computed(() => {
  return convertFileSrc(uiStore.currentBackgroundMusic)
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
      soundEffectPlayer.value.src = newAudioUrl
      soundEffectPlayer.value.load()
      soundEffectPlayer.value.play()
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

<template>
  <!-- 背景图，已使用 Tailwind 类替代原本的 css -->
  <ImageAcrossFade
    ref="imageFadeRef"
    class="game-background"
    :src="uiStore.currentBackground || '@/assets/images/default_bg.jpg'"
    position="center center"
    object-fit="cover"
    :duration="1000"
  >
    <StarField
      ref="starfieldRef"
      v-if="uiStore.currentBackgroundEffect === 'StarField'"
      :enabled="starfieldEnabled"
      :star-count="starCount"
      :scroll-speed="scrollSpeed"
      :colors="starColors"
      @ready="onStarfieldReady"
    />
    <Rain
      v-if="uiStore.currentBackgroundEffect === 'Rain'"
      :enabled="rainEnabled"
      :intensity="rainIntensity"
    />
    <Sakura v-if="uiStore.currentBackgroundEffect === 'Sakura'" :enabled="true" :intensity="1.5" />
    <Snow
      v-if="uiStore.currentBackgroundEffect === 'Snow'"
      :intensity="snowIntensity"
      :enabled="true"
    />
    <Fireworks
      v-if="uiStore.currentBackgroundEffect === 'Fireworks'"
      :enabled="true"
      :intensity="1.5"
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
    @ended="handleTrackEnd"
  />
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useUIStore } from '../../../stores/modules/ui/ui'
import ImageAcrossFade from '@/components/ui/ImageAcrossFade.vue'
import AudioAcrossFade from '@/components/ui/AudioAcrossFade.vue' // 引入组件
import StarField from './particles/StarField.vue'
import Rain from './particles/Rain.vue'
import Sakura from './particles/Sakura.vue'
import Snow from './particles/Snow.vue'
import Fireworks from './particles/Fireworks.vue'

const uiStore = useUIStore()

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

<template>
  <!-- 背景图 + 背景光照滤镜 -->
  <div
    v-if="backgroundSrc"
    class="absolute inset-0"
    :style="bgLightingFilter"
  >
    <ImageAcrossFade
      ref="imageFadeRef"
      class="game-background"
      :src="backgroundSrc"
      position="center center"
      object-fit="cover"
      :duration="uiStore.currentBackgroundTransition"
    />
  </div>

  <!-- 粒子特效层：独立于背景图，透明背景时依然显示 -->
  <!-- isolation: isolate 建立独立层叠上下文，把粒子组件内部的 z-index 限制在本层内，
       防止其逃逸到根层叠上下文盖过 UI 面板 -->
  <div class="pointer-events-none absolute inset-0" style="isolation: isolate">
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
  </div>

  <!-- 背景光照叠加层（在背景上方、角色下方） -->
  <div
    v-if="bgOverlayStyle"
    class="pointer-events-none absolute inset-0"
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
    :rate="uiStore.bgMusicPlaybackRate"
    :duration="800"
    :loop="uiStore.bgMusicMode === 'loop-single'"
    @ended="handleTrackEnd"
  />

  <!-- 环境音多轨渲染（每轨独立交叉淡入淡出循环组件，最多8轨并行） -->
  <AmbientLoopPlayer
    v-for="d in displayTracks"
    :key="d.id"
    :src="srcOf(d)"
    :volume="d.volume"
    :loop="d.loop"
    :fade="d.fade"
    :paused="d.paused"
    :stopped="d.stopped"
    @stopped-done="onStoppedDone(d.id)"
  />
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { toPlayableMediaUrl } from '@/utils/mediaUrl'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useGameStore } from '../../../stores/modules/game'
import ImageAcrossFade from '@/components/ui/ImageAcrossFade.vue'
import AudioAcrossFade from '@/components/ui/AudioAcrossFade.vue'
import AmbientLoopPlayer from '@/components/ui/AmbientLoopPlayer.vue'
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
    return bg || ''
  }
  return convertFileSrc(bg)
})

// 统一转换入口：currentBackgroundMusic 存储原始路径，在此一次性转换
// Android 上 asset 协议有 1MB 响应上限（见 toPlayableMediaUrl），需整文件 fetch 成 blob
const backgroundMusicSrc = ref('None')
let bgmLoadSeq = 0
watch(
  () => uiStore.currentBackgroundMusic,
  async (src) => {
    const seq = ++bgmLoadSeq
    if (!src || src === 'None') {
      backgroundMusicSrc.value = 'None'
      return
    }
    try {
      const url = await toPlayableMediaUrl(src)
      // 竞态守卫：期间已切换过歌曲则丢弃过期结果
      if (seq !== bgmLoadSeq) return
      backgroundMusicSrc.value = url
    } catch (e) {
      if (seq !== bgmLoadSeq) return
      console.warn('背景音乐加载失败:', src, e)
      backgroundMusicSrc.value = 'None'
    }
  },
  { immediate: true },
)

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
// currentSoundEffect 存原始路径（与 music/ambient 同一约定），在此统一转换
watch(
  () => uiStore.currentSoundEffect,
  (rawPath: string | null | undefined) => {
    if (soundEffectPlayer.value && rawPath && rawPath !== 'None') {
      // 重置 src 确保相同路径的重复事件也能触发播放
      soundEffectPlayer.value.pause()
      soundEffectPlayer.value.currentTime = 0
      soundEffectPlayer.value.src = ''
      void toPlayableMediaUrl(rawPath)
        .then((url) => {
          if (!soundEffectPlayer.value) return
          soundEffectPlayer.value.src = url
          soundEffectPlayer.value.load()
          soundEffectPlayer.value.play().catch(() => {})
        })
        .catch((e) => console.warn('音效加载失败:', rawPath, e))
    }
  },
)

// !!! 在此处：因为把背景音乐交给了 AudioCrossFade 组件，所以原先的大段背景音乐逻辑全被彻底删除。

// ========== 环境音多轨管理 ==========
// 显示用轨道镜像：保留淡出期间的"离场"轨道，避免直接卸载造成点击
const displayTracks = ref<
  Array<{
    id: string
    src: string
    volume: number // 0-1 有效音量
    loop: boolean
    fade: boolean
    paused: boolean
    stopped: boolean
  }>
>([])

const effectiveVolume = (trackVolume: number) => (trackVolume / 100) * (uiStore.ambientVolume / 100)

// Android 上 asset 协议有 1MB 响应上限，整文件 fetch 成 blob 后替换 src（见 toPlayableMediaUrl）
const playableSrcMap = ref(new Map<string, string>())
const resolvePlayableSrc = async (d: { id: string; src: string }) => {
  try {
    playableSrcMap.value.set(d.id, await toPlayableMediaUrl(d.src))
  } catch (e) {
    console.warn('环境音加载失败:', d.src, e)
  }
}

const srcOf = (d: { src: string; id: string }) =>
  playableSrcMap.value.get(d.id) ?? (d.src.startsWith('blob:') ? d.src : convertFileSrc(d.src))

// 同步 store 轨道到 displayTracks：新增则挂载，移除则标记离场触发淡出
watch(
  () => uiStore.ambientTracks,
  (newTracks) => {
    const storeIds = new Set(newTracks.map((t) => t.id))
    // 新增或同步已有（非离场）
    for (const t of newTracks) {
      const d = displayTracks.value.find((x) => x.id === t.id)
      if (!d) {
        const entry = {
          id: t.id,
          src: t.src,
          volume: effectiveVolume(t.volume),
          loop: t.loop,
          fade: t.fade ?? true,
          paused: t.paused ?? false,
          stopped: false,
        }
        displayTracks.value.push(entry)
        // Android 上异步把 src 换成 blob URL（完成前组件先以 asset URL 挂载，失败会被 src 更新覆盖）
        void resolvePlayableSrc(entry)
      } else if (!d.stopped) {
        d.src = t.src
        d.volume = effectiveVolume(t.volume)
        d.loop = t.loop
        d.fade = t.fade ?? true
        d.paused = t.paused ?? false
      }
    }
    // store 中已移除的标记离场，由组件淡出后回调清理
    for (const d of displayTracks.value) {
      if (!storeIds.has(d.id) && !d.stopped) d.stopped = true
    }
  },
  { deep: true },
)

// 全局环境音音量变化时，刷新所有非离场轨道音量
watch(
  () => uiStore.ambientVolume,
  (newVol) => {
    if (newVol == null) return
    for (const d of displayTracks.value) {
      if (d.stopped) continue
      const t = uiStore.ambientTracks.find((x) => x.id === d.id)
      if (t) d.volume = (t.volume / 100) * (newVol / 100)
    }
  },
)

// 组件淡出完成回调：从 displayTracks 移除并卸载
const onStoppedDone = (id: string) => {
  const idx = displayTracks.value.findIndex((x) => x.id === id)
  if (idx >= 0) displayTracks.value.splice(idx, 1)
  // 同步清理已解析的播放 URL，避免残留
  playableSrcMap.value.delete(id)
}
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

<template>
  <div class="absolute w-full h-full overflow-hidden">
    <!-- 1. 所有 Live2D 角色共享一个场景级 Pixi Application -->
    <Live2DStage
      class="z-2"
      :roles="gameStore.presentRolesList"
      mode="standard"
      :active-speaker-id="gameStore.currentInteractRoleId"
      :audio-element="mainAudio"
      :voice-data-url="voiceDataUrl"
    >
      <!-- 2. 每个角色保留原有静态视觉、气泡和触摸层 -->
      <RoleAvatar
        v-for="role in gameStore.presentRolesList"
        :key="role.roleId"
        :role="role"
      />
    </Live2DStage>

    <!-- 3. 场景光照叠加层 -->
    <div
      v-if="lightOverlayStyle"
      class="absolute inset-0 pointer-events-none z-10"
      :style="lightOverlayStyle as any"
    ></div>

    <!-- 4. 全局主语音播放器 -->
    <audio ref="mainAudio" @ended="onAudioEnded"></audio>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { getVoiceAudio } from '@/api/services/game-info'
import { setVoicePlaying } from '@/composables/useAsrInput'
import RoleAvatar from './GameRoleAvatar.vue'
import Live2DStage from '../live2d/Live2DStage.vue'

const gameStore = useGameStore()
const uiStore = useUIStore()
const emit = defineEmits(['audio-ended', 'audio-started'])

const mainAudio = ref<HTMLAudioElement | null>(null)
const voiceDataUrl = ref('')
let voicePlaybackSeq = 0

type PitchControllableAudio = HTMLAudioElement & {
  mozPreservesPitch?: boolean
  webkitPreservesPitch?: boolean
}

/** 恐怖剧本变速必须同时改变音高；显式关闭各 WebView 的“保持音高”。 */
const applyVoiceRate = (audio: HTMLAudioElement, rate: number) => {
  const controllable = audio as PitchControllableAudio
  controllable.preservesPitch = false
  controllable.mozPreservesPitch = false
  controllable.webkitPreservesPitch = false
  controllable.playbackRate = rate > 0 ? rate : 1
}

// ---------- Web Audio 纯降调路径（voice_shift 的 pitch != 0 时启用） ----------
// AudioBufferSourceNode.detune 以音分（1/100 半音）为单位做纯音调偏移，不改变语速；
// 与 playbackRate（变速变调）可叠加。HTMLAudioElement 无此能力，故 pitch 模式走这里。
let audioCtx: AudioContext | null = null
let bufferSource: AudioBufferSourceNode | null = null
let gainNode: GainNode | null = null
const audioBufferCache = new Map<string, AudioBuffer>()

const getAudioCtx = (): AudioContext => {
  if (!audioCtx) audioCtx = new AudioContext()
  return audioCtx
}

const loadVoiceBuffer = async (dataUrl: string): Promise<AudioBuffer> => {
  const cached = audioBufferCache.get(dataUrl)
  if (cached) return cached
  const resp = await fetch(dataUrl)
  const buf = await getAudioCtx().decodeAudioData(await resp.arrayBuffer())
  audioBufferCache.set(dataUrl, buf)
  return buf
}

const stopWebAudio = () => {
  if (bufferSource) {
    bufferSource.onended = null
    try {
      bufferSource.stop()
    } catch {
      // 未 start 或已停止：忽略
    }
    bufferSource.disconnect()
    bufferSource = null
  }
  gainNode = null
}

const playVoiceWithPitch = async (dataUrl: string, volume: number, playbackSeq: number) => {
  stopWebAudio()
  const buf = await loadVoiceBuffer(dataUrl)
  if (playbackSeq !== voicePlaybackSeq) return
  const ctx = getAudioCtx()
  if (ctx.state === 'suspended') await ctx.resume().catch(() => {})
  const src = ctx.createBufferSource()
  src.buffer = buf
  // 半音 → 音分
  src.detune.value = uiStore.voicePitch * 100
  src.playbackRate.value = uiStore.voiceRate > 0 ? uiStore.voiceRate : 1
  const gain = ctx.createGain()
  gain.gain.value = volume
  src.connect(gain).connect(ctx.destination)
  src.onended = () => {
    if (bufferSource === src) bufferSource = null
    onAudioEnded()
  }
  bufferSource = src
  gainNode = gain
  setVoicePlaying(true)
  src.start()
  emit('audio-started')
}

const lightOverlayStyle = computed(() => {
  const l = gameStore.currentScene?.lighting
  if (!l?.overlay_enabled) return undefined
  if (l.overlay_target !== 'character' && l.overlay_target !== 'both') return undefined
  const blend = l.blend_mode !== 'normal' ? l.blend_mode : 'overlay'
  return `background: radial-gradient(circle at ${l.light_x}% ${l.light_y}%, ${l.overlay_color1} 0%, ${l.overlay_color2} ${l.overlay_radius}%); mix-blend-mode: ${blend}; opacity: ${l.overlay_opacity}`
})

// --- 音频逻辑 (全局) ---
// 监听 UI Store 的音频播放指令
watch(
  () => uiStore.currentAvatarAudio,
  async (newAudio) => {
    if (!mainAudio.value) return
    const playbackSeq = ++voicePlaybackSeq

    // 如果设置为 'None'，停止当前播放
    if (newAudio === 'None' || !newAudio) {
      voiceDataUrl.value = ''
      mainAudio.value.pause()
      mainAudio.value.currentTime = 0
      stopWebAudio()
      setVoicePlaying(false)
      return
    }

    if (newAudio && newAudio !== 'None') {
      try {
        const dataUrl = await getVoiceAudio(newAudio)
        if (playbackSeq !== voicePlaybackSeq) return
        // 最新 Live2D 口型同步消费 voiceDataUrl；DLC 的 voice_shift 仍按需走 Web Audio。
        voiceDataUrl.value = dataUrl
        const volume = uiStore.characterVolume / 100
        if (uiStore.voicePitch !== 0) {
          // 纯降调模式：走 Web Audio detune（不变速），与 HTMLAudio 路径互斥
          mainAudio.value.pause()
          mainAudio.value.currentTime = 0
          await playVoiceWithPitch(dataUrl, volume, playbackSeq)
          return
        }
        stopWebAudio()
        mainAudio.value.src = dataUrl
        mainAudio.value.load()
        mainAudio.value.volume = volume
        // 剧本 voice_shift 恶魔音：降低播放倍率并关闭保音高。
        applyVoiceRate(mainAudio.value, uiStore.voiceRate)
        // TTS 播放中 ASR 禁用（外放 TTS 进麦克风会误识别 AI 自己的话）。
        mainAudio.value
          .play()
          .then(() => {
            setVoicePlaying(true)
            emit('audio-started')
          })
          .catch((e) => {
            console.error('播放失败', e)
            setVoicePlaying(false)
          })
      } catch (e) {
        console.error('获取语音文件失败:', e)
        if (playbackSeq === voicePlaybackSeq) setVoicePlaying(false)
      }
    }
  },
)

watch(
  () => uiStore.characterVolume,
  (v) => {
    if (mainAudio.value) mainAudio.value.volume = v / 100
    if (gainNode) gainNode.gain.value = v / 100
  },
)

// voice_shift 发生在当前一句播放途中时也立即生效；后续新语音会在加载时再次应用。
watch(
  () => uiStore.voiceRate,
  (rate) => {
    if (mainAudio.value) applyVoiceRate(mainAudio.value, rate)
    if (bufferSource) bufferSource.playbackRate.value = rate > 0 ? rate : 1
  },
)

// 音调偏移（半音 → 音分）在播放途中同样即时生效
watch(
  () => uiStore.voicePitch,
  (pitch) => {
    if (bufferSource) bufferSource.detune.value = pitch * 100
  },
)

const onAudioEnded = () => {
  setVoicePlaying(false)
  emit('audio-ended')
}

// 暴露停止音频的方法给父组件
const stopAudio = () => {
  voicePlaybackSeq += 1
  if (mainAudio.value) {
    mainAudio.value.pause()
    mainAudio.value.currentTime = 0
    setVoicePlaying(false)
  }
  stopWebAudio()
}

defineExpose({
  stopAudio,
})
</script>

<style scoped></style>

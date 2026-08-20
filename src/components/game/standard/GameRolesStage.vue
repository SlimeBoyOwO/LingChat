<template>
  <div class="absolute w-full h-full overflow-hidden">
    <!-- 1. 遍历渲染所有在场角色 -->
    <RoleAvatar
      v-for="(role, index) in gameStore.presentRolesList"
      :key="role.roleId"
      :role="role"
      :live2d-active="live2dActiveRoleIds.has(role.roleId)"
      :live2d-failed="live2dFailedRoleIds.has(role.roleId)"
    />

    <Live2DStage
      v-if="hasLive2dRoles"
      class="z-2"
      :roles="gameStore.presentRolesList"
      mode="standard"
      :active-speaker-id="gameStore.currentInteractRoleId"
      :audio-element="mainAudio"
      :voice-data-url="voiceDataUrl"
      @active-change="setLive2dActiveRoles"
      @failed-change="setLive2dFailedRoles"
    />

    <!-- 2. 场景光照叠加层 -->
    <div
      v-if="lightOverlayStyle"
      class="absolute inset-0 pointer-events-none z-10"
      :style="lightOverlayStyle as any"
    ></div>

    <!-- 3. 全局主语音播放器 -->
    <audio ref="mainAudio" @ended="onAudioEnded"></audio>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { getVoiceAudio } from '@/api/services/game-info'
import RoleAvatar from './GameRoleAvatar.vue'
import Live2DStage from '../live2d/Live2DStage.vue'

const gameStore = useGameStore()
const uiStore = useUIStore()
const emit = defineEmits(['audio-ended', 'audio-started'])

const mainAudio = ref<HTMLAudioElement | null>(null)
const voiceDataUrl = ref('')
const live2dActiveRoleIds = ref(new Set<number>())
const live2dFailedRoleIds = ref(new Set<number>())
const hasLive2dRoles = computed(() => gameStore.presentRolesList.some((role) => Boolean(role.live2d)))

const setLive2dActiveRoles = (roleIds: number[]) => {
  live2dActiveRoleIds.value = new Set(roleIds)
}

const setLive2dFailedRoles = (roleIds: number[]) => {
  live2dFailedRoleIds.value = new Set(roleIds)
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

    // 如果设置为 'None'，停止当前播放
    if (newAudio === 'None' || !newAudio) {
      voiceDataUrl.value = ''
      mainAudio.value.pause()
      mainAudio.value.currentTime = 0
      return
    }

    if (newAudio && newAudio !== 'None') {
      try {
        const dataUrl = await getVoiceAudio(newAudio)
        voiceDataUrl.value = dataUrl
        mainAudio.value.src = dataUrl
        mainAudio.value.load()
        mainAudio.value.volume = uiStore.characterVolume / 100
        mainAudio.value.play().catch((e) => console.error('播放失败', e))
        emit('audio-started')
      } catch (e) {
        console.error('获取语音文件失败:', e)
      }
    }
  },
)

watch(
  () => uiStore.characterVolume,
  (v) => {
    if (mainAudio.value) mainAudio.value.volume = v / 100
  },
)

const onAudioEnded = () => {
  emit('audio-ended')
}

// 暴露停止音频的方法给父组件
const stopAudio = () => {
  if (mainAudio.value) {
    mainAudio.value.pause()
    mainAudio.value.currentTime = 0
  }
}

defineExpose({
  stopAudio,
})
</script>

<style scoped></style>

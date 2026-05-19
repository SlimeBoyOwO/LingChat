<template>
  <RoleAvatar
    v-if="singleRole"
    :key="singleRole.roleId"
    :role="singleRole"
    @avatar-click="handleAvatarClick"
    @open-settings="handleOpenSettings"
    @switch-auto-mode="handleSwitchAutoMode"
    @exit-pet-mode="handleExitPetMode"
  />

  <audio ref="mainAudio" @ended="onAudioEnded"></audio>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { getVoiceAudio } from '@/api/services/game-info'
import RoleAvatar from './GameRoleAvatar.vue'

const gameStore = useGameStore()
const uiStore = useUIStore()
const emit = defineEmits([
  'audio-ended',
  'audio-started',
  'avatar-click',
  'open-settings',
  'switch-auto-mode',
  'exit-pet-mode',
])

const mainAudio = ref<HTMLAudioElement | null>(null)

const singleRole = computed(() => {
  return gameStore.presentRolesList.length > 0 ? gameStore.presentRolesList[0] : null
})

watch(
  () => uiStore.currentAvatarAudio,
  async (newAudio) => {
    if (!mainAudio.value) return

    if (newAudio === 'None' || !newAudio) {
      mainAudio.value.pause()
      mainAudio.value.currentTime = 0
      return
    }

    try {
      const dataUrl = await getVoiceAudio(newAudio)
      mainAudio.value.src = dataUrl
      mainAudio.value.load()
      mainAudio.value.play().catch((e) => console.error('播放失败', e))
      emit('audio-started')
    } catch (e) {
      console.error('获取语音文件失败:', e)
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

const handleAvatarClick = () => {
  emit('avatar-click')
}

const handleOpenSettings = () => {
  emit('open-settings')
}

const handleSwitchAutoMode = () => {
  emit('switch-auto-mode')
}

const handleExitPetMode = () => {
  emit('exit-pet-mode')
}
</script>

<style scoped></style>

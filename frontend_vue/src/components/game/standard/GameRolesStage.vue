<template>
  <div class="absolute w-full h-full overflow-hidden">
    <!-- 1. 遍历渲染所有在场角色 -->
    <!-- z-index 可以根据 index 动态设置，保证后面渲染的在上面，或者根据 y轴 排序 -->
    <RoleAvatar
      v-for="(role, index) in gameStore.presentRolesList"
      :key="role.roleId"
      :role="role"
    />

    <!-- 2. 全局主语音播放器 -->
    <!-- 将语音逻辑放在父级，因为通常同一时间只有一段对话语音 -->
    <audio ref="mainAudio" @ended="onAudioEnded"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { API_CONFIG } from '@/controllers/core/config'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import RoleAvatar from './GameRoleAvatar.vue'

const gameStore = useGameStore()
const uiStore = useUIStore()
const emit = defineEmits(['audio-ended'])

const mainAudio = ref<HTMLAudioElement | null>(null)

// --- 音频逻辑 (全局) ---
// 监听 UI Store 的音频播放指令
watch(
  () => uiStore.currentAvatarAudio,
  (newAudio) => {
    if (mainAudio.value && newAudio && newAudio !== 'None') {
      mainAudio.value.src = `${API_CONFIG.VOICE.BASE}/${newAudio}`
      mainAudio.value.load()

      // 可以在这里判断是谁在说话，做一些特殊处理，例如让当前角色"动"一下
      // const speakerId = gameStore.currentInteractRoleId

      mainAudio.value.play().catch((e) => console.error('播放失败', e))
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
</script>

<style scoped></style>

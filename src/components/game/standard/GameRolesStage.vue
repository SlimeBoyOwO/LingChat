<template>
  <div class="absolute w-full h-full overflow-hidden">
    <!-- 1. 遍历渲染所有在场角色 -->
    <RoleAvatar
      v-for="(role, index) in gameStore.presentRolesList"
      :key="role.roleId"
      :role="role"
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

const gameStore = useGameStore()
const uiStore = useUIStore()
const emit = defineEmits(['audio-ended', 'audio-started'])

const mainAudio = ref<HTMLAudioElement | null>(null)

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
      mainAudio.value.pause()
      mainAudio.value.currentTime = 0
      return
    }

    if (newAudio && newAudio !== 'None') {
      try {
        const dataUrl = await getVoiceAudio(newAudio)
        mainAudio.value.src = dataUrl
        mainAudio.value.load()
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

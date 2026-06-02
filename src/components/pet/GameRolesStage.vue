<template>
  <div class="relative flex items-center justify-center w-full h-full group">
    <!-- 缩放与尺寸控制层 (无位移) -->
    <div
      class="relative transition-transform duration-300 ease-out animate-pet-scale"
      :style="{ width: frameSize + 'px', height: frameSize + 'px' }"
    >
      <!-- 设置按钮 -->
      <button
        type="button"
        aria-label="打开设置"
        title="设置"
        class="absolute top-1 -left-3.5 z-40 w-8 h-8 rounded-full bg-neutral-950/60 backdrop-blur-xl border border-white/10 text-white flex items-center justify-center opacity-0 translate-y-2 group-hover:opacity-100 group-hover:translate-y-0 hover:bg-cyan-500/80 hover:text-white hover:scale-110 shadow-[0_4px_12px_rgba(0,0,0,0.3)] transition-all duration-300"
        @click.stop="handleOpenSettings"
      >
        <Settings :size="16" />
      </button>

      <!-- 自动按钮 -->
      <button
        type="button"
        aria-label="打开自动对话"
        title="自动"
        class="absolute top-10 -left-3.5 z-40 w-8 h-8 rounded-full bg-neutral-950/60 backdrop-blur-xl border border-white/10 text-white flex items-center justify-center opacity-0 translate-y-2 group-hover:opacity-100 group-hover:translate-y-0 hover:bg-cyan-500/80 hover:text-white hover:scale-110 shadow-[0_4px_12px_rgba(0,0,0,0.3)] transition-all duration-300"
        :class="{ '!bg-cyan-500/80 !border-cyan-400/50': uiStore.autoMode }"
        @click.stop="handleSwitchAutoMode"
      >
        <Play v-if="!uiStore.autoMode" :size="16" />
        <Pause v-else :size="16" />
      </button>

      <!-- 返回主页按钮 -->
      <button
        type="button"
        aria-label="返回主页"
        title="返回主页"
        class="absolute top-19 -left-3.5 z-40 w-8 h-8 rounded-full bg-neutral-950/60 backdrop-blur-xl border border-white/10 text-white flex items-center justify-center opacity-0 translate-y-2 group-hover:opacity-100 group-hover:translate-y-0 hover:bg-cyan-500/80 hover:text-white hover:scale-110 shadow-[0_4px_12px_rgba(0,0,0,0.3)] transition-all duration-300"
        @click.stop="handleExitPetMode"
      >
        <LogOut :size="16" />
      </button>

      <!-- 截图按钮 -->
      <div
        class="absolute top-28 -left-3.5 z-40 opacity-0 translate-y-2 group-hover:opacity-100 group-hover:translate-y-0 transition-all duration-300"
      >
        <button
          type="button"
          :title="hasScreenshot ? '点击重新截图，右键取消截图' : '截图提问'"
          class="w-8 h-8 rounded-full bg-neutral-950/60 backdrop-blur-xl border border-white/10 text-white flex items-center justify-center hover:bg-cyan-500/80 hover:text-white hover:scale-110 shadow-[0_4px_12px_rgba(0,0,0,0.3)] transition-all duration-300"
          :style="
            hasScreenshot
              ? { color: 'var(--accent-color)', borderColor: 'var(--accent-color)' }
              : {}
          "
          @click.stop="startScreenshot"
          @contextmenu.prevent="clearScreenshot"
        >
          <Camera :size="16" />
        </button>
      </div>

      <!-- 角色头像 -->
      <RoleAvatar
        v-if="singleRole"
        :key="singleRole.roleId"
        :role="singleRole"
        @avatar-click="emit('avatar-click')"
      />
    </div>

    <audio ref="mainAudio" @ended="onAudioEnded"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { getVoiceAudio } from '@/api/services/game-info'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useSettingsStore } from '@/stores/modules/settings'
import { useScreenshot } from '@/composables/useScreenshot'
import RoleAvatar from './GameRoleAvatar.vue'
import { Play, Pause, Settings, LogOut, Camera } from 'lucide-vue-next'

const gameStore = useGameStore()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()

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

const frameSize = computed(() => {
  const scale = settingsStore.pet?.scale || 1
  return Math.round(210 * scale)
})

// --- 截图 ---
const {
  hasScreenshot,
  init: initScreenshot,
  destroy: destroyScreenshot,
  start: startScreenshot,
  clear: clearScreenshot,
} = useScreenshot()

onMounted(() => initScreenshot())
onUnmounted(() => destroyScreenshot())

// --- 音频 ---
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

// --- 按钮事件 ---
const handleOpenSettings = () => emit('open-settings')
const handleSwitchAutoMode = () => emit('switch-auto-mode')
const handleExitPetMode = () => emit('exit-pet-mode')
</script>

<style scoped>
.animate-pet-scale {
  animation: pet-scale-in 0.4s ease-out;
}

@keyframes pet-scale-in {
  0% {
    transform: scale(0.8);
    opacity: 0;
  }
  100% {
    transform: scale(1);
    opacity: 1;
  }
}
</style>

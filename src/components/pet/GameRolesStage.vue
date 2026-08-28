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
        :aria-label="$t('views.pet.stage.openSettingsAria')"
        :title="$t('views.pet.stage.settings')"
        class="absolute top-1 -left-3.5 z-40 w-8 h-8 rounded-full bg-neutral-950/60 backdrop-blur-xl border border-white/10 text-white flex items-center justify-center opacity-0 translate-y-2 group-hover:opacity-100 group-hover:translate-y-0 hover:bg-cyan-500/80 hover:text-white hover:scale-110 shadow-[0_4px_12px_rgba(0,0,0,0.3)] transition-all duration-300"
        @click.stop="handleOpenSettings"
      >
        <Settings :size="16" />
      </button>

      <!-- 自动按钮 -->
      <button
        type="button"
        :aria-label="$t('views.pet.stage.openAutoAria')"
        :title="$t('views.pet.stage.auto')"
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
        :aria-label="$t('views.pet.stage.backHome')"
        :title="$t('views.pet.stage.backHome')"
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
          :title="titleText"
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

<!-- 语音输入按钮（与桌面 GameDialog 同源：useAsrInput 共享会话） -->
      <div
        class="absolute top-37 -left-3.5 z-40 opacity-0 translate-y-2 group-hover:opacity-100 group-hover:translate-y-0 transition-all duration-300"
      >
        <button
          type="button"
          :title="micTitle"
          :disabled="!canStartMic"
          class="w-8 h-8 rounded-full bg-neutral-950/60 backdrop-blur-xl border border-white/10 text-white flex items-center justify-center hover:bg-cyan-500/80 hover:text-white hover:scale-110 shadow-[0_4px_12px_rgba(0,0,0,0.3)] transition-all duration-300 disabled:opacity-40 disabled:cursor-not-allowed"
          :class="{
            '!text-blue-400 !border-blue-400/50 !bg-blue-950/40 animate-asr-breathe':
              asrInput.phase.value === 'recording',
          }"
          :style="
            !asrInput.phase.value && autoListenOn && !autoListenActive
              ? { color: 'var(--accent-color)', borderColor: 'var(--accent-color)' }
              : {}
          "
          @click.stop="toggleRecording"
        >
          <component :is="micIcon" :size="16" />
        </button>
      </div>

      <!-- Live2D 角色渲染（上游合并） -->
      <Live2DStage
        v-if="singleRole?.live2d"
        class="z-11 rounded-full"
        :roles="singleRole ? [singleRole] : []"
        mode="pet"
        :active-speaker-id="gameStore.currentInteractRoleId"
        :audio-element="mainAudio"
        :voice-data-url="voiceDataUrl"
        @active-change="setLive2dActiveRoles"
        @failed-change="setLive2dFailedRoles"
      />

      <!-- 角色头像 -->
      <RoleAvatar
        v-if="singleRole"
        :key="singleRole.roleId"
        :role="singleRole"
        :live2d-active="live2dActiveRoleIds.has(singleRole.roleId)"
        :live2d-failed="live2dFailedRoleIds.has(singleRole.roleId)"
        @avatar-click="emit('avatar-click')"
      />
    </div>

    <audio ref="mainAudio" @ended="onAudioEnded"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { getVoiceAudio } from '@/api/services/game-info'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useSettingsStore } from '@/stores/modules/settings'
import { useScreenshot } from '@/composables/useScreenshot'
import { useAsrStore } from '@/stores/modules/settings/asr'
import { useAsrInput, setVoicePlaying } from '@/composables/useAsrInput'
import { isAndroid } from '@/utils/platform'
import RoleAvatar from './GameRoleAvatar.vue'
import Live2DStage from '../game/live2d/Live2DStage.vue'
import { Play, Pause, Settings, LogOut, Camera, Mic, MicOff } from 'lucide-vue-next'

const { t } = useI18n()
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
const voiceDataUrl = ref('')
const live2dActiveRoleIds = ref(new Set<number>())
const live2dFailedRoleIds = ref(new Set<number>())

const setLive2dActiveRoles = (roleIds: number[]) => {
  live2dActiveRoleIds.value = new Set(roleIds)
}

const setLive2dFailedRoles = (roleIds: number[]) => {
  live2dFailedRoleIds.value = new Set(roleIds)
}

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

const titleText = computed(() => {
  if (isAndroid()) {
    return hasScreenshot.value
      ? t('views.pet.stage.retakePhoto')
      : t('views.pet.stage.photoOrImage')
  }
  return hasScreenshot.value
    ? t('views.pet.stage.retakeScreenshot')
    : t('views.pet.stage.screenshotAsk')
})

onMounted(() => initScreenshot())
onUnmounted(() => destroyScreenshot())

// --- 音频 ---
watch(
  () => uiStore.currentAvatarAudio,
  async (newAudio) => {
    if (!mainAudio.value) return

    if (newAudio === 'None' || !newAudio) {
      voiceDataUrl.value = ''
      mainAudio.value.pause()
      mainAudio.value.currentTime = 0
      setVoicePlaying(false)
      return
    }

    try {
      const dataUrl = await getVoiceAudio(newAudio)
      voiceDataUrl.value = dataUrl
      mainAudio.value.src = dataUrl
      mainAudio.value.load()
      mainAudio.value.volume = uiStore.characterVolume / 100
      // TTS 播放中 ASR 禁用（外放 TTS 进麦克风会误识别 AI 自己的话）
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
  setVoicePlaying(false)
  emit('audio-ended')
}

// --- 语音输入（与桌面 GameDialog 同源：useAsrInput 模块级单例共享会话） ---
const asrInput = useAsrInput()
const asrStore = useAsrStore()

// 三层状态（与 GameDialog 一致）：auto_listen 模式开 + 总开关开 → mic = 功能开关；
// 总开关关（自动模式已停）→ 退化为手动录音
const autoListenOn = computed(() => asrStore.settings.auto_listen)
const autoListenActive = computed(() => asrInput.autoListenActive.value)
const micIcon = computed(() => {
  if (autoListenOn.value && asrStore.settings.voice_input_enabled) {
    return autoListenActive.value ? MicOff : Mic
  }
  return Mic
})
const micTitle = computed(() => {
  if (autoListenOn.value && asrStore.settings.voice_input_enabled) {
    return autoListenActive.value
      ? t('game.dialog.asrAutoOff') // 监听中：暂停
      : t('game.dialog.asrAutoResume') // 已暂停：恢复
  }
  return asrInput.phase.value === 'recording'
    ? t('game.dialog.recordingStop')
    : t('game.dialog.voiceInput')
})
// mic 按钮 enabled 条件（与 GameDialog 一致）：
// - auto_listen 模式开 + 总开关开：功能开关可用
// - 总开关关 → 整体禁用（总开关是语音输入的总闸，手动 mic 一并关闭；
//   显示锁只挡 auto 触发，手动不受限）
const canStartMic = computed(
  () =>
    (autoListenOn.value && asrStore.settings.voice_input_enabled) ||
    asrInput.phase.value === 'recording' ||
    asrInput.canStartAsr(false, true),
)
function toggleRecording() {
  // auto_listen 模式开 + 总开关开：mic 按钮 = 切换功能开关（暂停/恢复监听），
  // 不改模式设置；总开关关 → 走手动录音分支
  if (autoListenOn.value && asrStore.settings.voice_input_enabled) {
    asrInput.toggleAutoListenFunction()
    return
  }
  if (asrInput.phase.value === 'idle') {
    void asrInput.start('button').catch(() => {
      /* 会话忙时静默忽略 */
    })
  } else if (asrInput.phase.value === 'recording') {
    asrInput.stop()
  }
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

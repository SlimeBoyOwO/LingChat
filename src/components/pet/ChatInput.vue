<template>
  <div
    class="relative w-full z-10 flex justify-center transition-all duration-300 ease-out"
    :class="
      props.visible ? 'opacity-100 translate-y-0' : 'opacity-0 -translate-y-2 pointer-events-none'
    "
    :style="{ '--pet-ui-scale': scale }"
  >
    <div
      class="flex items-center p-[calc(4px*var(--pet-ui-scale,1))] rounded-[calc(20px*var(--pet-ui-scale,1))] bg-neutral-950/50 backdrop-blur-xl saturate-200 border border-white/10 chat-input-container"
    >
      <input
        v-model="messageText"
        type="text"
        :placeholder="placeholderText"
        :readonly="!isInputEnabled"
        class="flex-1 bg-transparent border-none outline-none text-white text-[calc(13px*var(--pet-ui-scale,1))] p-[calc(5px*var(--pet-ui-scale,1))] placeholder-white/40 [text-shadow:0_1px_4px_rgba(0,0,0,0.5)]"
        @keyup.enter="sendMessage"
        @compositionstart="isCompsing = true"
        @compositionend="isCompsing =false"
      />
      <!-- 语音输入（与桌面 GameDialog 同源：useAsrInput 共享会话） -->
      <button
        class="h-6 px-2 bg-white/10 hover:bg-white/20 text-white rounded-full transition-colors flex items-center gap-1 shrink-0 disabled:opacity-40 disabled:cursor-not-allowed"
        :disabled="!canStartMic"
        :title="micTitle"
        @click="toggleRecording"
      >
        <component :is="micIcon" class="w-3.5 h-3.5" />
      </button>
      <button
        class="h-6 px-2 bg-linear-to-tr from-cyan-500 to-blue-400 hover:from-cyan-400 hover:to-blue-300 text-white font-bold text-sm rounded-full shadow-[0_4px_15px_rgba(6,182,212,0.4)] hover:shadow-[0_6px_20px_rgba(6,182,212,0.6)] transition-all duration-300 active:scale-95 flex items-center gap-1 overflow-hidden relative"
        @click="sendMessage"
        :disabled="!isInputEnabled"
      >
        <div
          class="absolute top-0 left-0 w-full h-1/2 bg-white/20 rounded-t-full pointer-events-none"
        ></div>
        <Forward />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useSettingsStore } from '@/stores/modules/settings'
import { useLlmProvidersStore } from '@/stores/modules/llm-providers'
import { useAsrStore } from '@/stores/modules/settings/asr'
import { useAsrInput, registerAsrInputBridge } from '@/composables/useAsrInput'
import { useScreenshot } from '@/composables/useScreenshot'
import { setInputHasText } from '@/composables/useCanDeliver'
import { Forward, Mic, MicOff } from 'lucide-vue-next'

const { t } = useI18n()
const gameStore = useGameStore()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()
const llmStore = useLlmProvidersStore()

const {
  screenshotBase64,
  init: initScreenshot,
  destroy: destroyScreenshot,
  clear: clearScreenshot,
} = useScreenshot()

// ── 语音输入（与桌面 GameDialog 同源：useAsrInput 模块级单例共享会话） ──
const asrInput = useAsrInput()
const asrStore = useAsrStore()

// 三层状态（与 GameDialog 一致）：auto_listen 模式开 → mic = 功能开关
const autoListenOn = computed(() => asrStore.settings.auto_listen)
const autoListenActive = computed(() => asrInput.autoListenActive.value)
const micIcon = computed(() => {
  if (autoListenOn.value) return autoListenActive.value ? MicOff : Mic
  return Mic
})
const micTitle = computed(() => {
  if (autoListenOn.value) {
    return autoListenActive.value
      ? t('game.dialog.asrAutoOff') // 监听中：暂停
      : t('game.dialog.asrAutoResume') // 已暂停：恢复
  }
  return asrInput.phase.value === 'recording'
    ? t('game.dialog.recordingStop')
    : t('game.dialog.voiceInput')
})
const canStartMic = computed(
  () =>
    asrStore.settings.voice_input_enabled &&
    (autoListenOn.value || asrInput.phase.value === 'recording' || asrInput.canStartAsr()),
)
function toggleRecording() {
  // auto_listen 模式开：mic 按钮 = 切换功能开关（暂停/恢复监听），不改模式设置
  if (autoListenOn.value) {
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

// fill_only：识别完成整句填入（与桌面 onAsrText 一致）
function onAsrText(e: Event) {
  const ce = e as CustomEvent<string>
  if (typeof ce.detail === 'string') {
    messageText.value = ce.detail
  }
}

onMounted(() => {
  initScreenshot()
  // 输入桥：流式 partial 实时写入（只在录音发起窗口生效——phase 是窗口本地状态）
  registerAsrInputBridge({
    getText: () => messageText.value,
    setText: (v) => {
      messageText.value = v
    },
  })
  window.addEventListener('asr-text', onAsrText)
})
onUnmounted(() => {
  window.removeEventListener('asr-text', onAsrText)
  destroyScreenshot()
})

const scale = computed(() => settingsStore.pet?.scale || 1.0)

const placeholderText = computed(() => {
  switch (gameStore.currentStatus) {
    case 'input':
      return uiStore.showPlayerHintLine || t('views.pet.chatInput.placeholder')
    case 'thinking':
      const currentInteractRole = gameStore.currentInteractRole
      if (currentInteractRole) {
        const baseMessage = currentInteractRole.thinkMessage
        if (gameStore.thinkingLength > 0) {
          return t('views.pet.chatInput.deepThought', {
            message: baseMessage,
            length: gameStore.thinkingLength,
          })
        }
        return baseMessage
      } else {
        return t('views.pet.chatInput.waiting')
      }
    case 'responding':
      return t('views.pet.chatInput.chatting')
    case 'presenting':
      return ''
    default:
      return t('views.pet.chatInput.placeholderDefault')
  }
})

watch(
  () => gameStore.currentStatus,
  (newStatus) => {
    console.log('游戏状态变为 :', newStatus)
    if (newStatus === 'thinking') {
      const currentInteractRole = gameStore.currentInteractRole
      if (currentInteractRole) {
        // 思考态不再写入 'AI思考' 伪情感，避免立绘组件因 emotion 残留而无法加载
        uiStore.showCharacterTitle = currentInteractRole.roleName
        uiStore.showCharacterSubtitle = currentInteractRole.roleSubTitle
      }
    } else if (newStatus === 'input') {
      uiStore.showCharacterEmotion = ''
    }
  },
)

const isInputEnabled = computed(() => gameStore.currentStatus === 'input')

const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
})

const emit = defineEmits(['message-sent'])

const messageText = ref('')
// 输入框内容变化 → 通知 can_deliver 追踪
watch(messageText, (val) => setInputHasText(Boolean(val.trim())), { immediate: true })

const isCompsing = ref(false)
const isTyping = () => messageText.value.trim() != '' || isCompsing.value
defineExpose({ isTyping })

const sendMessage = () => {
  const text = messageText.value.trim()
  if (!text) return

  // 检查对话模型是否已选择
  if (!llmStore.chatProviderId) {
    uiStore.showNotification({
      type: 'warning',
      title: t('views.pet.chatInput.noModelTitle'),
      message: t('views.pet.chatInput.noModelMessage'),
      skipTipsCheck: true,
    })
    return
  }

  if (gameStore.runningScript) {
    invoke('script_submit_input', { input: text }).catch((error) => {
      console.error('发送脚本输入失败:', error)
      gameStore.currentStatus = 'input'
    })
    gameStore.runningScript.choices = []
    if (gameStore.runningScript.freeDialogueInfo.isFreeDialogue) {
      gameStore.runningScript.freeDialogueInfo.currentRound++
    }
  } else {
    invoke('send_chat_message', { text, screenshotBase64: screenshotBase64.value }).catch(
      (error) => {
        console.error('发送消息失败:', error)
        gameStore.currentStatus = 'input'
      },
    )
  }

  emit('message-sent', text)
  messageText.value = ''
  clearScreenshot()
}
</script>

<style scoped></style>

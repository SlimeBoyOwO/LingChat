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
import {
  useAsrInput,
  registerAsrInputBridge,
  lockAsrForDisplay,
  ASR_AUTO_SEND_DELAY_MS,
} from '@/composables/useAsrInput'
import { useScreenshot } from '@/composables/useScreenshot'
import { setInputHasText } from '@/composables/useCanDeliver'
import { Forward } from 'lucide-vue-next'

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

// fill_only：识别完成整句填入（与桌面 onAsrText 一致）；400ms 显示锁防
// auto RMS 识别完立即再触发覆盖刚填入的内容（手动不受锁限）
const ASR_DISPLAY_MS = 400
function onAsrText(e: Event) {
  const ce = e as CustomEvent<string>
  if (typeof ce.detail === 'string') {
    messageText.value = ce.detail
    lockAsrForDisplay(ASR_DISPLAY_MS)
  }
}

// auto_send：识别结果显示到输入框 → ASR_AUTO_SEND_DELAY_MS 后走 sendMessage()
//（完整复用剧本分支/模型检查/输入框清理；显示锁已由 handle() 设置）
function onAsrAutoSend(e: Event) {
  const ce = e as CustomEvent<string>
  if (typeof ce.detail !== 'string') return
  messageText.value = ce.detail
  window.setTimeout(() => sendMessage(), ASR_AUTO_SEND_DELAY_MS)
}

// 输入桥：流式 partial 实时写入（与桌面 GameDialog 一致；录音发起窗口的
// phase 是窗口本地状态，partial 只写入发起方输入框）
const asrInput = useAsrInput()
onMounted(() => {
  initScreenshot()
  registerAsrInputBridge({
    getText: () => messageText.value,
    setText: (v) => {
      messageText.value = v
    },
  })
  window.addEventListener('asr-text', onAsrText)
  window.addEventListener('asr-send', onAsrAutoSend)
})
onUnmounted(() => {
  window.removeEventListener('asr-text', onAsrText)
  window.removeEventListener('asr-send', onAsrAutoSend)
  destroyScreenshot()
})

// 与主对话窗口行为一致（GameDialog 同款 watch）：AI 回复（showCharacterLine
// 非空 + responding）时清空输入框——auto_send 识别文本填入后随回复自动清空
watch([() => uiStore.showCharacterLine, () => gameStore.currentStatus], ([newLine, newStatus]) => {
  if (newLine && newLine !== '' && newStatus === 'responding') {
    messageText.value = ''
  }
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

<template>
  <div
    class="relative w-full z-10 flex justify-center transition-all duration-300 ease-out"
    :class="
      props.visible ? 'opacity-100 translate-y-0' : 'opacity-0 -translate-y-2 pointer-events-none'
    "
    :style="{ '--pet-ui-scale': scale }"
  >
    <div
      class="flex items-center p-[calc(4px*var(--pet-ui-scale,1))] rounded-[calc(20px*var(--pet-ui-scale,1))] bg-white/20 backdrop-blur-[10px] saturate-180 border border-white/20 shadow-[0_8px_32px_rgba(0,0,0,0.1),inset_0_1px_1px_rgba(255,255,255,0.1)] chat-input-container"
    >
      <input
        v-model="messageText"
        type="text"
        :placeholder="placeholderText"
        :readonly="!isInputEnabled"
        class="flex-1 bg-transparent border-none outline-none text-white text-[calc(13px*var(--pet-ui-scale,1))] p-[calc(5px*var(--pet-ui-scale,1))] placeholder-white/60"
        @keyup.enter="sendMessage"
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
import { invoke } from '@tauri-apps/api/core'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useSettingsStore } from '@/stores/modules/settings'
import { useScreenshot } from '@/composables/useScreenshot'
import { Forward } from 'lucide-vue-next'

const gameStore = useGameStore()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()

const {
  screenshotBase64,
  init: initScreenshot,
  destroy: destroyScreenshot,
  clear: clearScreenshot,
} = useScreenshot()

onMounted(() => initScreenshot())
onUnmounted(() => destroyScreenshot())

const scale = computed(() => settingsStore.pet?.scale || 1.0)

const placeholderText = computed(() => {
  switch (gameStore.currentStatus) {
    case 'input':
      return uiStore.showPlayerHintLine || '输入消息...'
    case 'thinking':
      const currentInteractRole = gameStore.currentInteractRole
      if (currentInteractRole) {
        return currentInteractRole.thinkMessage
      } else {
        return '等待回应中...'
      }
    case 'responding':
      return '聊天ing~'
    case 'presenting':
      return ''
    default:
      return '在这里输入消息...'
  }
})

watch(
  () => gameStore.currentStatus,
  (newStatus) => {
    console.log('游戏状态变为 :', newStatus)
    if (newStatus === 'thinking') {
      const currentInteractRole = gameStore.currentInteractRole
      if (currentInteractRole) {
        currentInteractRole.emotion = 'AI思考'
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

const sendMessage = () => {
  const text = messageText.value.trim()
  if (!text) return

  gameStore.appendGameMessage({
    type: 'message',
    displayName: gameStore.userName,
    content: text,
  })

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

<style scoped>
</style>

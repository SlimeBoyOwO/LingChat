<template>
  <div
    class="relative flex justify-center w-full z-2 p-3.75 backdrop-blur-[1px] transition-all duration-2000 ease-[cubic-bezier(0.25,0.46,0.45,0.94)] bg-linear-to-t from-[rgba(0,14,39,0.7)] to-[rgba(0,14,39,0.6)] before:content-[''] before:absolute before:-top-10 before:left-0 before:right-0 before:h-10 before:bg-linear-to-b before:from-transparent before:via-[rgba(0,14,39,0.3)] before:to-[rgba(0,14,39,0.6)] before:pointer-events-none [scrollbar-width:thin] [scrollbar-color:var(--accent-color)_transparent]"
    :class="{
      'opacity-0 z-[-1]! overflow-hidden duration-500! ease-linear before:opacity-0 before:duration-1000!':
        isHidden,
    }"
  >
    <div class="w-[60%]">
      <div class="flex items-baseline mb-2.5">
        <div class="text-[24px] font-bold text-white mr-3.75 font-[inherit] text-shadow-[inherit]">
          <div id="character">{{ uiStore.showCharacterTitle }}</div>
        </div>
        <div class="text-[20px] font-bold text-[#6eb4ff] font-[inherit] text-shadow-[inherit]">
          <div id="character-sub">{{ uiStore.showCharacterSubtitle }}</div>
        </div>
        <div class="text-[20px] font-bold text-[#ff77dd] m-auto">
          <div id="character-emotion">{{ uiStore.showCharacterEmotion }}</div>
        </div>

        <!-- 操作按钮组 -->
        <Button type="nav" icon="background" title="场景设置" @click="openSceneSettings"></Button>
        <Button
          type="nav"
          icon="hand"
          title="触摸模式"
          @click="toggleTouchMode"
          @contextmenu.prevent="exitTouchMode"
        ></Button>
        <Button type="nav" icon="history" title="历史记录" @click="openHistory"></Button>

        <!-- 新增：语音输入按钮 (已将 icon 修复为 mic) -->
        <Button
          type="nav"
          icon="mic"
          :title="isRecording ? '录音中，点击停止' : '语音输入'"
          :class="{ 'text-red-500 animate-pulse': isRecording }"
          @click="toggleRecording"
        ></Button>

        <Button type="nav" icon="close" title="关闭对话" @click="removeDialog"></Button>
      </div>

      <!-- 分割线 -->
      <div class="h-px bg-white/30 my-1.5"></div>

      <!-- 输入框区域 -->
      <div
        class="flex flex-col whitespace-pre-line w-full min-h-10 bg-transparent border-none text-white text-[20px] font-bold resize-none my-1.25 outline-none transition-all duration-300"
      >
        <textarea
          id="inputMessage"
          ref="textareaRef"
          class="w-full min-h-10 bg-transparent border-none text-white text-[20px] font-bold resize-none my-1.25 outline-none transition-all duration-300 placeholder:text-white/50 placeholder:shadow-none font-[inherit] text-shadow-[inherit]"
          :placeholder="placeholderText"
          v-model="inputMessage"
          @keydown.enter.exact.prevent="sendOrContinue"
          :readonly="!isInputEnabled"
        ></textarea>
        <button
          id="sendButton"
          class="self-end bg-transparent text-[#04bcff] border-none px-2.5 py-1 rounded-[5px] cursor-pointer transition-all duration-300 text-[20px] font-bold scale-x-150 hover:bg-transparent hover:text-[rgba(136,255,251,0.827)] disabled:bg-[#333] disabled:cursor-not-allowed disabled:opacity-70 font-[inherit] text-shadow-[inherit]"
          :disabled="isSending"
          @click="sendOrContinue"
        >
          ▼
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onUnmounted } from 'vue'
import { Button } from '../../base'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useTypeWriter } from '../../../composables/ui/useTypeWriter'
import { eventQueue } from '../../../core/events/event-queue'
import { scriptHandler } from '../../../api/websocket/handlers/script-handler'

const inputMessage = ref('')
const textareaRef = ref<HTMLTextAreaElement | null>(null)
const gameStore = useGameStore()
const uiStore = useUIStore()
const isHidden = ref(false)

// 语音识别相关状态
const isRecording = ref(false)
const interimText = ref('') // 新增：用于实时存储临时识别出来的文本
let speechRecognition: any = null

const openSceneSettings = () => {
  uiStore.toggleSettings(true)
  uiStore.setSettingsTab('background')
}
const currentDisplayedText = ref('')

const { startTyping, stopTyping } = useTypeWriter(textareaRef, (text) => {
  currentDisplayedText.value = text
})

const isSending = computed(() => gameStore.currentStatus === 'thinking')
const isTyping = computed(
  () =>
    uiStore.showCharacterLine !== '' && currentDisplayedText.value !== uiStore.showCharacterLine,
)

const emit = defineEmits(['player-continued', 'dialog-proceed'])

const openHistory = () => {
  uiStore.toggleSettings(true)
  uiStore.setSettingsTab('history')
}

const handleRightClick = (e: MouseEvent) => {
  if (gameStore.command === 'touch') {
    e.preventDefault()
    exitTouchMode()
  }
}

const handleDialogShow = (e: MouseEvent) => {
  if (isHidden.value) {
    e.preventDefault()
    isHidden.value = false
  }
}

const toggleTouchMode = () => {
  if (gameStore.command === 'touch') {
    exitTouchMode()
  } else {
    document.body.style.cursor = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' class='lucide lucide-hand-icon lucide-hand'%3E%3Cpath d='M18 11V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2'/%3E%3Cpath d='M14 10V4a2 2 0 0 0-2-2a2 2 0 0 0-2 2v2'/%3E%3Cpath d='M10 10.5V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2v8'/%3E%3Cpath d='M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-5.99-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15'/%3E%3C/svg%3E") 0 0, auto`
    gameStore.command = 'touch'
    document.addEventListener('contextmenu', handleRightClick)
  }
}

const exitTouchMode = () => {
  document.body.style.cursor = 'default'
  gameStore.command = null
  document.removeEventListener('contextmenu', handleRightClick)
}

const placeholderText = computed(() => {
  // 如果正在录音，优先展示实时的语音内容，如果没有内容则展示正在聆听
  if (isRecording.value) {
    return interimText.value || '正在聆听...'
  }

  switch (gameStore.currentStatus) {
    case 'input':
      return uiStore.showPlayerHintLine || '在这里输入消息...'
    case 'thinking':
      const currentInteractRole = gameStore.currentInteractRole
      if (currentInteractRole) {
        return currentInteractRole.thinkMessage
      } else {
        return '等待回应中...'
      }
    case 'responding':
    case 'presenting':
      return ''
    default:
      return '在这里输入消息...'
  }
})

const isInputEnabled = computed(() => gameStore.currentStatus === 'input')

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
      uiStore.showCharacterTitle = gameStore.userName
      uiStore.showCharacterSubtitle = gameStore.userSubtitle
      uiStore.showCharacterEmotion = ''
    } else if (newStatus === 'presenting') {
      uiStore.showCharacterTitle = ''
      uiStore.showCharacterSubtitle = ''
      uiStore.showCharacterEmotion = ''
      uiStore.showCharacterLine = ''
    }
  },
)

watch([() => uiStore.showCharacterLine, () => gameStore.currentStatus], ([newLine, newStatus]) => {
  if (newLine && newLine !== '' && newStatus === 'responding') {
    inputMessage.value = ''
    currentDisplayedText.value = ''
    startTyping(newLine, uiStore.typeWriterSpeed)
  } else if (newStatus === 'input') {
    stopTyping()
    inputMessage.value = ''
    currentDisplayedText.value = ''
  }
})

// === 语音识别功能实现 ===
const initSpeechRecognition = () => {
  const SpeechRecognition =
    (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition
  if (!SpeechRecognition) {
    console.warn('当前浏览器不支持 Web Speech API，语音功能不可用')
    return null
  }

  const recognition = new SpeechRecognition()
  recognition.lang = 'zh-CN' // 默认识别中文
  // 修改：将 interimResults 设为 true 以获取中间结果
  recognition.interimResults = true
  recognition.maxAlternatives = 1

  recognition.onstart = () => {
    isRecording.value = true
    interimText.value = '' // 开始录音时清空中间文本
  }

  recognition.onresult = (event: any) => {
    let interim = ''
    let final = ''

    // 遍历所有结果，区分是最终结果还是正在识别的临时结果
    for (let i = event.resultIndex; i < event.results.length; ++i) {
      if (event.results[i].isFinal) {
        final += event.results[i][0].transcript
      } else {
        interim += event.results[i][0].transcript
      }
    }

    if (interim) {
      // 如果有中间结果，更新到专门的变量供 placeholder 使用
      interimText.value = interim
    }

    if (final) {
      // 识别完成，赋值并发送
      interimText.value = ''
      inputMessage.value = final
      send()
    }
  }

  recognition.onerror = (event: any) => {
    console.error('语音识别出错:', event.error)
    isRecording.value = false
    interimText.value = ''
  }

  recognition.onend = () => {
    isRecording.value = false
    interimText.value = ''
  }

  return recognition
}

const toggleRecording = () => {
  if (!speechRecognition) {
    alert('您的浏览器不支持语音输入功能，建议使用最新版的 Chrome 或 Edge 浏览器。')
    return
  }
  if (isRecording.value) {
    speechRecognition.stop()
  } else {
    // 如果不在允许输入的阶段，阻止录音
    if (gameStore.currentStatus !== 'input') {
      alert('当前状态不允许输入，请稍候再试。')
      return
    }
    speechRecognition.start()
  }
}

onMounted(() => {
  document.addEventListener('contextmenu', handleDialogShow)
  // 初始化语音识别对象
  speechRecognition = initSpeechRecognition()
})

onUnmounted(() => {
  document.removeEventListener('contextmenu', handleDialogShow)
})

function sendOrContinue() {
  if (gameStore.currentStatus === 'input') {
    send()
  } else if (gameStore.currentStatus === 'responding') {
    continueDialog(true)
  }
}

function send() {
  const text = inputMessage.value
  if (!text.trim()) return
  if (text === '/开始剧本') {
    // gameStore.initializeScript('TODO: 从剧本面板选择剧本')
  } else {
    gameStore.appendGameMessage({
      type: 'message',
      displayName: gameStore.userName,
      content: text,
    })
  }

  scriptHandler.sendMessage(text)

  if (gameStore.runningScript) {
    gameStore.runningScript.choices = []
  }

  inputMessage.value = ''
}

function continueDialog(isPlayerTrigger: boolean): boolean {
  const needWait = eventQueue.continue()
  if (!needWait) {
    if (isPlayerTrigger) emit('player-continued')
    emit('dialog-proceed')
  }

  return needWait
}

function removeDialog(e: Event) {
  isHidden.value = true
}

defineExpose({
  continueDialog,
  isTyping,
})
</script>

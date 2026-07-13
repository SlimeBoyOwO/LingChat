<template>
  <div
    class="relative flex justify-center w-full z-2 p-3.75 backdrop-blur-[1px] transition-all duration-200 ease-[cubic-bezier(0.25,0.46,0.45,0.94)] bg-linear-to-t from-[rgba(0,14,39,0.7)] to-[rgba(0,14,39,0.6)] before:content-[''] before:absolute before:-top-10 before:left-0 before:right-0 before:h-10 before:bg-linear-to-b before:from-transparent before:via-[rgba(0,14,39,0.3)] before:to-[rgba(0,14,39,0.6)] before:pointer-events-none scrollbar-thin [scrollbar-color:var(--accent-color)_transparent]"
    :class="{
      'opacity-0 z-[-1]! overflow-hidden duration-500! ease-linear before:opacity-0 before:duration-1000!':
        isHidden,
      'max-h-[40vh]': !uiStore.isNarrowScreen,
    }"
  >
    <div :style="{ width: containerWidth + '%' }" class="relative">
      <div class="overflow-y-auto flex flex-col">
        <div class="flex items-baseline mb-2 shrink-0">
          <!-- 角色名称（窄屏时可截断，为右侧按钮腾出空间） -->
          <div
            class="text-2xl font-bold text-white mr-3.75 font-[inherit] text-shadow-[inherit]"
            :class="{
              'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap': uiStore.isNarrowScreen,
            }"
          >
            <div id="character">{{ uiStore.showCharacterTitle }}</div>
          </div>
          <div
            v-show="!uiStore.isNarrowScreen"
            class="text-xl font-bold text-[#6eb4ff] font-[inherit] text-shadow-[inherit]"
          >
            <div id="character-sub">{{ uiStore.showCharacterSubtitle }}</div>
          </div>

          <div
            class="text-xl font-bold text-[#ff77dd] font-[inherit] text-shadow-[inherit] shrink-0"
            :class="uiStore.isNarrowScreen ? 'ml-auto' : 'ml-3'"
          >
            <div id="character-emotion">{{ uiStore.showCharacterEmotion }}</div>
          </div>

          <div
            class="flex items-baseline ml-auto min-w-0"
            :class="{ 'flex-1 shrink-0': uiStore.isNarrowScreen }"
          >

            <!-- 桌面端：直接显示所有操作按钮 -->
            <template v-if="!isMobile">
              <!-- 操作按钮组（窄屏时占据右侧容器剩余空间，可横向滚动） -->
              <div
                class="overflow-x-auto custom-scroll"
                :class="uiStore.isNarrowScreen ? 'flex-1 min-w-0' : 'shrink-0'"
              >
                <div class="flex whitespace-nowrap">
                  <Button
                    type="nav"
                    icon="background"
                    title="场景设置"
                    @click="openSceneSettings"
                  ></Button>
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

                  <div class="relative inline-flex group">
                    <div
                      v-if="hasScreenshot"
                      class="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 opacity-0 group-hover:opacity-100 transition-opacity duration-200 pointer-events-none z-50"
                    >
                      <img
                        :src="'data:image/jpeg;base64,' + screenshotBase64"
                        class="max-w-96 max-h-64 rounded-lg shadow-lg border-2 object-contain"
                        style="border-color: var(--accent-color); background: #000"
                      />
                    </div>
                    <Button
                      type="nav"
                      icon="camera"
                      :title="hasScreenshot ? '点击重新截图，右键取消截图' : '截图提问'"
                      :style="hasScreenshot ? { color: 'var(--accent-color)' } : {}"
                      @click="startScreenshot"
                      @contextmenu.prevent="clearScreenshot"
                    ></Button>
                  </div>

                  <Button type="nav" icon="close" title="关闭对话" @click="removeDialog"></Button>
                </div>
              </div>
            </template>

            <!-- 移动端：箭头折叠按钮 -->
            <button
              v-if="isMobile"
              class="mobile-toggle-btn"
              :class="{ 'is-open': showMobileMenu }"
              title="更多操作"
              @click="showMobileMenu = !showMobileMenu"
            >
              ▲
            </button>

            <!-- 关闭按钮始终可见 -->
            <Button type="nav" icon="close" title="关闭对话" @click="removeDialog"></Button>
          </div>

          <!-- 移动端：折叠菜单下拉面板 -->
          <Transition name="mobile-menu">
            <div
              v-if="isMobile && showMobileMenu"
              class="mobile-menu-dropdown"
            >
              <Button type="nav" icon="background" title="场景设置" @click="onMobileMenuAction(openSceneSettings)"></Button>
              <Button
                type="nav"
                icon="hand"
                title="触摸模式"
                @click="onMobileMenuAction(toggleTouchMode)"
                @contextmenu.prevent="exitTouchMode"
              ></Button>
              <Button type="nav" icon="history" title="历史记录" @click="onMobileMenuAction(openHistory)"></Button>
              <Button
                type="nav"
                icon="mic"
                :title="isRecording ? '录音中，点击停止' : '语音输入'"
                :class="{ 'text-red-500 animate-pulse': isRecording }"
                @click="onMobileMenuAction(toggleRecording)"
              ></Button>
              <div class="relative inline-flex group">
                <div
                  v-if="hasScreenshot"
                  class="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 opacity-0 group-hover:opacity-100 transition-opacity duration-200 pointer-events-none z-50"
                >
                  <img
                    :src="'data:image/jpeg;base64,' + screenshotBase64"
                    class="max-w-96 max-h-64 rounded-lg shadow-lg border-2 object-contain"
                    style="border-color: var(--accent-color); background: #000"
                  />
                </div>
                <Button
                  type="nav"
                  icon="camera"
                  :title="hasScreenshot ? '点击重新截图，右键取消截图' : '截图提问'"
                  :style="hasScreenshot ? { color: 'var(--accent-color)' } : {}"
                  @click="onMobileMenuAction(startScreenshot)"
                  @contextmenu.prevent="onMobileMenuAction(clearScreenshot)"
                ></Button>
              </div>
            </div>
          </Transition>
        </div>

        <!-- 分割线 -->
        <div class="h-px bg-white/30 my-1.5 shrink-0"></div>

        <!-- 输入区 -->
        <div
          class="flex flex-1 min-h-30 whitespace-pre-line w-full bg-transparent border-none text-xl font-bold my-1.25 outline-none transition-all duration-300"
        >
          <!-- 内联动作文本显示区（仅内联模式+回应状态时可见） -->
          <div
            v-show="isInlineDisplayMode"
            ref="inlineDisplayRef"
            tabindex="0"
            class="inline-motion-display flex-1 min-h-30 max-h-[50vh] bg-transparent border-none text-xl font-bold resize-none my-1.25 outline-none whitespace-pre-line font-[inherit] text-shadow-[inherit] overflow-y-auto"
            @keydown.enter.exact.prevent="sendOrContinue"
          ></div>

          <!-- 标准 textarea（输入模式或非内联显示模式） -->
          <textarea
            v-show="!isInlineDisplayMode"
            id="inputMessage"
            ref="textareaRef"
            class="flex-1 min-h-30 max-h-[50vh] bg-transparent border-none text-white text-xl font-bold resize-none my-1.25 outline-none transition-all duration-300 placeholder:text-white/50 placeholder:shadow-none font-[inherit] text-shadow-[inherit]"
            :class="textareaMotionClass"
            :placeholder="placeholderText"
            v-model="inputMessage"
            @keydown.enter.exact.prevent="sendOrContinue"
            :readonly="!isInputEnabled"
          ></textarea>
        </div>
      </div>
      <!-- 发送按钮（内层右侧外部） -->
      <button
        id="sendButton"
        class="absolute right-0 bottom-0 translate-x-full bg-transparent text-[#04bcff] border-none px-2 py-2 rounded-[5px] cursor-pointer transition-all duration-300 text-sm font-bold hover:bg-transparent hover:text-[rgba(136,255,251,0.827)] disabled:bg-[#333] disabled:cursor-not-allowed disabled:opacity-70 font-[inherit] text-shadow-[inherit]"
        :disabled="isSending"
        @click="sendOrContinue"
      >
        ▼
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onUnmounted } from 'vue'
import { Button } from '../../base'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useDialogStore } from '../../../stores/modules/ui/dialog'
import { useSettingsStore } from '../../../stores/modules/settings'
import { useTypeWriter } from '../../../composables/ui/useTypeWriter'
import { escapeHtml } from '../../../utils/escapeHtml'
import { eventQueue } from '../../../core/events/event-queue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { setInputHasText } from '../../../composables/useCanDeliver'

const inputMessage = ref('')
// 输入框内容变化 → 通知 can_deliver 追踪
watch(inputMessage, (val) => setInputHasText(Boolean(val.trim())), { immediate: true })
const isShowingMotionText = ref(false)
const textareaRef = ref<HTMLTextAreaElement | null>(null)
const inlineDisplayRef = ref<HTMLDivElement | null>(null)
const gameStore = useGameStore()
const uiStore = useUIStore()
const dialogStore = useDialogStore()
const settingsStore = useSettingsStore()
const isHidden = ref(false)

// 移动端按钮折叠状态
const isMobile = ref(window.innerWidth <= 768)
const showMobileMenu = ref(false)

// 内联显示模式：设置开启 + 回应状态 → 用 div 做混色显示
const isInlineDisplayMode = computed(
  () => settingsStore.text.inlineMotionText && gameStore.currentStatus === 'responding',
)

// 语音识别相关状态
const isRecording = ref(false)
const interimText = ref('') // 新增：用于实时存储临时识别出来的文本
let speechRecognition: any = null

// 截图相关状态
const hasScreenshot = ref(false)
const screenshotBase64 = ref<string | null>(null)
const isCapturing = ref(false)

// 响应式容器宽度（窄屏判断从 uiStore 读取）
const containerWidth = ref(60)

const updateContainerWidth = () => {
  containerWidth.value = Math.max(60, uiStore.aspectRatio > 1 ? 70 : 90)
  isMobile.value = window.innerWidth <= 768
  if (!isMobile.value) showMobileMenu.value = false
}

const openSceneSettings = () => {
  uiStore.toggleSettings(true)
  uiStore.setSettingsTab('background')
}

// 移动端菜单操作：执行动作后自动收起菜单
const onMobileMenuAction = (action: () => void) => {
  action()
  showMobileMenu.value = false
}
const currentDisplayedText = ref('')

/**
 * 内联显示写入函数：根据文本中 \n 的位置构建混色 innerHTML。
 * 换行前 → 白色 span，换行后 → 灰色 span（.motion-text-gray）。
 */
function writeInlineHtml(_element: HTMLElement, text: string): void {
  if (!inlineDisplayRef.value) return
  const newlineIndex = text.indexOf('\n')
  if (newlineIndex > 0) {
    const dialogue = escapeHtml(text.substring(0, newlineIndex))
    const motion = escapeHtml(text.substring(newlineIndex + 1))
    inlineDisplayRef.value.innerHTML =
      `<span style="color:#fff">${dialogue}</span><br><span class="motion-text-gray">${motion}</span>`
  } else if (newlineIndex === 0) {
    const motion = escapeHtml(text.substring(1))
    inlineDisplayRef.value.innerHTML =
      `<br><span class="motion-text-gray">${motion}</span>`
  } else {
    inlineDisplayRef.value.innerHTML =
      `<span style="color:#fff">${escapeHtml(text)}</span>`
  }
}

// 标准模式 TypeWriter（textarea）
const {
  startTyping: startTextTyping,
  stopTyping: stopTextTyping,
  isTyping: isTextTyping,
} = useTypeWriter(textareaRef, (text) => {
  currentDisplayedText.value = text
})

// 内联模式 TypeWriter（div + HTML 混色渲染）
const {
  startTyping: startInlineTyping,
  stopTyping: stopInlineTyping,
  isTyping: isInlineTyping,
  finishTyping: finishInlineTyping,
} = useTypeWriter(inlineDisplayRef, (text) => {
  currentDisplayedText.value = text
}, writeInlineHtml)

// 统一 isTyping（父组件通过 defineExpose 使用）
const isTyping = computed(() =>
  isInlineDisplayMode.value ? isInlineTyping.value : isTextTyping.value,
)

const isSending = computed(() => gameStore.currentStatus === 'thinking')

// textarea 动态样式（仅两段式模式使用；内联模式用 div 渲染，不需要此 class）
const textareaMotionClass = computed(() => {
  if (!isShowingMotionText.value) return {}
  return { 'italic text-white/50 text-base': true }
})

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
        const baseMessage = currentInteractRole.thinkMessage
        if (gameStore.thinkingLength > 0) {
          return `${baseMessage}（已深度思考 ${gameStore.thinkingLength} 字）`
        }
        return baseMessage
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
    isShowingMotionText.value = false

    // 内联模式：始终用 div 渲染（有动作文本时拼接换行+灰字，无则仅白字）
    if (settingsStore.text.inlineMotionText) {
      const text = uiStore.showCharacterMotionText
        ? newLine + '\n' + uiStore.showCharacterMotionText
        : newLine
      startInlineTyping(text, uiStore.typeWriterSpeed)
    } else {
      startTextTyping(newLine, uiStore.typeWriterSpeed)
    }
  } else if (newStatus === 'input') {
    stopTextTyping()
    stopInlineTyping()
    isShowingMotionText.value = false
    inputMessage.value = ''
    currentDisplayedText.value = ''
  }
})

// 内联模式 div 可见时自动聚焦，确保 Enter 键能推进对话
watch(isInlineDisplayMode, (visible) => {
  if (visible) {
    // nextTick 确保 v-show 已生效、DOM 已渲染
    setTimeout(() => inlineDisplayRef.value?.focus(), 0)
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

const toggleRecording = async () => {
  if (!speechRecognition) {
    await dialogStore.alert(
      '您的浏览器不支持语音输入功能，建议使用最新版的 Chrome 或 Edge 浏览器。',
    )
    return
  }
  if (isRecording.value) {
    speechRecognition.stop()
  } else {
    // 如果不在允许输入的阶段，阻止录音
    if (gameStore.currentStatus !== 'input') {
      await dialogStore.alert('当前状态不允许输入，请稍候再试。')
      return
    }
    speechRecognition.start()
  }
}

let unlistenScreenshot: (() => void) | null = null
let unlistenCancelled: (() => void) | null = null

onMounted(async () => {
  document.addEventListener('contextmenu', handleDialogShow)
  // 初始化语音识别对象
  speechRecognition = initSpeechRecognition()
  // 初始化容器宽度
  updateContainerWidth()
  // 监听窗口大小变化
  window.addEventListener('resize', updateContainerWidth)

  // 监听截图完成事件
  unlistenScreenshot = await listen<{ base64: string }>('screenshot:captured', (event) => {
    screenshotBase64.value = event.payload.base64
    hasScreenshot.value = true
    isCapturing.value = false
  })

  // 监听截图取消事件
  unlistenCancelled = await listen('screenshot:cancelled', () => {
    isCapturing.value = false
    hasScreenshot.value = false
  })
})

onUnmounted(() => {
  document.removeEventListener('contextmenu', handleDialogShow)
  window.removeEventListener('resize', updateContainerWidth)
  if (unlistenScreenshot) unlistenScreenshot()
  if (unlistenCancelled) unlistenCancelled()
})

async function startScreenshot() {
  if (isCapturing.value) return
  isCapturing.value = true
  try {
    await invoke('start_screenshot')
  } catch (error) {
    console.error('启动截图失败:', error)
    isCapturing.value = false
    await dialogStore.alert('截图功能初始化失败，请重试')
  }
}

function clearScreenshot() {
  if (hasScreenshot.value) {
    hasScreenshot.value = false
    screenshotBase64.value = null
  }
}

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

  gameStore.appendGameMessage({
    type: 'message',
    displayName: gameStore.userName,
    content: text,
  })

  // In script mode, submit input to the script engine; otherwise use chat
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
    invoke('send_chat_message', {
      text,
      screenshotBase64: screenshotBase64.value,
    }).catch((error) => {
      console.error('发送消息失败:', error)
      gameStore.currentStatus = 'input'
    })
  }

  // 发送后清除截图状态
  hasScreenshot.value = false
  screenshotBase64.value = null
  inputMessage.value = ''
}

function continueDialog(isPlayerTrigger: boolean): boolean {
  // 内联动作文本模式：第一次点击跳过打字动画，第二次推进事件队列
  if (settingsStore.text.inlineMotionText) {
    if (isInlineTyping.value) {
      finishInlineTyping()
      return false // 先跳到末尾，不推进
    }
    const needWait = eventQueue.continue()
    if (!needWait) {
      if (isPlayerTrigger) emit('player-continued')
      emit('dialog-proceed')
    }
    return needWait
  }

  // Phase 2: motion text already shown, now advance normally
  if (isShowingMotionText.value) {
    isShowingMotionText.value = false
    uiStore.showCharacterMotionText = ''
  }
  // Phase 1: there's pending motion text, show it instead of advancing
  else if (uiStore.showCharacterMotionText) {
    isShowingMotionText.value = true
    // Type the motion text into the same textarea with typewriter
    startTextTyping(uiStore.showCharacterMotionText, uiStore.typeWriterSpeed)
    return false // don't advance event queue
  }

  // Normal: advance to next event
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
  isTyping, // 统一 computed：内联模式用 div 实例，否则用 textarea 实例
})
</script>

<style scoped>
/* 内联显示 div：外观与 textarea 一致，支持 innerHTML 混色 */
.inline-motion-display {
  color: #9ca3af; /* fallback：极端情况下 div 直接显示文字时用灰色 */
}

/* 内联模式下的动作文本灰字 span（通过 writeInlineHtml 写入 innerHTML） */
.motion-text-gray {
  color: #9ca3af !important;
}

/* 兼容 Firefox */
.custom-scroll {
  scrollbar-width: thin;
}

/* 兼容 Chrome / Edge / Safari */
.custom-scroll::-webkit-scrollbar {
  width: 6px; /* 纵向滚动条宽度 */
  height: 6px; /* 横向滚动条高度（你这个是 overflow-x，主要控制这个） */
}

/* 移动端折叠按钮 — 与右侧 nav 关闭按钮等大 */
.mobile-toggle-btn {
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: white;
  border-radius: 8px;
  padding: 10px 14px;
  cursor: pointer;
  font-size: 16px;
  line-height: 1;
  transition: all 0.25s ease;
  margin: 0 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 38px;
}
.mobile-toggle-btn:hover {
  background: rgba(255, 255, 255, 0.2);
  color: var(--accent-color, #6eb4ff);
}
.mobile-toggle-btn:active {
  transform: scale(0.92);
}
.mobile-toggle-btn > span,
.mobile-toggle-btn {
  transition: transform 0.25s ease;
}
.mobile-toggle-btn.is-open {
  transform: rotate(180deg);
  background: rgba(255, 255, 255, 0.18);
  color: var(--accent-color, #6eb4ff);
  border-color: var(--accent-color, #6eb4ff);
}

/* 移动端下拉菜单 */
.mobile-menu-dropdown {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 8px 4px 4px;
  margin-top: 2px;
  border-top: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(0, 14, 39, 0.5);
  border-radius: 0 0 8px 8px;
}

/* Vue Transition: 移动端菜单展开/收起 */
.mobile-menu-enter-active {
  animation: menu-slide-down 0.2s ease-out;
}
.mobile-menu-leave-active {
  animation: menu-slide-down 0.15s ease-in reverse;
}
@keyframes menu-slide-down {
  from {
    opacity: 0;
    max-height: 0;
    padding-top: 0;
    padding-bottom: 0;
    margin-top: 0;
    border-top-width: 0;
  }
  to {
    opacity: 1;
    max-height: 200px;
    padding-top: 8px;
    padding-bottom: 4px;
    margin-top: 2px;
    border-top-width: 1px;
  }
}
</style>

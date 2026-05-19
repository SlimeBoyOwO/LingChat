<template>
  <div
    id="pet-app"
    :style="appStyleVars"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    class="relative w-[var(--app-width)] h-[var(--app-height)] flex flex-col justify-start items-center overflow-hidden transition-none select-none bg-transparent"
  >
    <!-- DialogueBox 区域 -->
    <div
      ref="dialogContainer"
      class="w-full shrink-0 flex items-end justify-center transition-none bg-transparent"
      :style="{ height: 'var(--dialog-h)' }"
    >
      <DialogueBox
        ref="gameDialogRef"
        @player-continued="manualTriggerContinue"
        @dialog-proceed="resetInteraction"
      />
    </div>

    <!-- Avatar 区域 -->
    <div
      ref="avatarContainer"
      class="shrink-0 flex items-center justify-center transition-all duration-100 bg-transparent"
      :style="{ width: 'var(--avatar-size)', height: 'var(--avatar-size)' }"
    >
      <GameRolesStage
        @avatar-click="handleAvatarClick"
        @open-settings="handleOpenSettings"
        @switch-auto-mode="handleSwitchAutoMode"
        @exit-pet-mode="handleExitPetMode"
        @audio-ended="handleAudioFinished"
        @audio-started="handleAudioStarted"
      />
    </div>

    <!-- ChatInput 区域 -->
    <div
      ref="chatContainer"
      class="w-full shrink-0 flex items-start justify-center transition-none bg-transparent"
      :style="{ height: 'var(--chat-h)' }"
    >
      <ChatInput :visible="showChatInput" @message-sent="handleMessageSent" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { useGameStore } from '@/stores/modules/game'
import { useSettingsStore } from '@/stores/modules/settings'
import { useUIStore } from '@/stores/modules/ui/ui'
import { eventQueue } from '@/core/events/event-queue'

import ChatInput from '../pet/ChatInput.vue'
import DialogueBox from '../pet/DialogueBox.vue'
import GameRolesStage from '../pet/GameRolesStage.vue'

const BASE_AVATAR_SIZE = 240
const CHAT_BASE_H = 45
const DIALOG_BASE_H = 75

const router = useRouter()
const gameStore = useGameStore()
const settingsStore = useSettingsStore()
const uiStore = useUIStore()

const showChatInput = ref(false)

const dialogContainer = ref<HTMLElement | null>(null)
const avatarContainer = ref<HTMLElement | null>(null)
const chatContainer = ref<HTMLElement | null>(null)
const gameDialogRef = ref<InstanceType<typeof DialogueBox> | null>(null)

const appStyleVars = computed(() => {
  const scale = settingsStore.pet?.scale || 1.0
  const layout = calcWindowLayout(scale)
  return {
    '--pet-ui-scale': scale.toString(),
    '--app-width': `${layout.width}px`,
    '--app-height': `${layout.height}px`,
    '--avatar-size': `${Math.round(BASE_AVATAR_SIZE * scale)}px`,
    '--chat-h': `${Math.round(CHAT_BASE_H * scale)}px`,
    '--dialog-h': `${Math.round(DIALOG_BASE_H * scale)}px`,
  }
})

const calcWindowLayout = (scale: number): { width: number; height: number } => {
  const S = Math.round(BASE_AVATAR_SIZE * scale)
  const chatH = Math.round(CHAT_BASE_H * scale)
  const dialogH = Math.round(DIALOG_BASE_H * scale)
  return { width: S, height: S + dialogH + chatH }
}

const applyWindowLayout = async () => {
  try {
    const scale = settingsStore.pet?.scale || 1.0
    await invoke('set_pet_mode', { enable: true, scale })
  } catch (error) {
    console.error('调整窗口布局失败:', error)
  }
}

let hitTestInterval: number | undefined
let scaleUnlisten: (() => void) | null = null
let effectUnlisten: (() => void) | null = null
let volumeUnlisten: (() => void) | null = null

onMounted(async () => {
  const appWindow = getCurrentWindow()

  scaleUnlisten = await appWindow.listen<{ scale: number }>(
    'pet-scale-changed',
    (event) => {
      const scale = Number(event.payload?.scale)
      if (!Number.isNaN(scale)) {
        settingsStore.pet.scale = scale
        void applyWindowLayout()
      }
    }
  )

  effectUnlisten = await appWindow.listen<{ effect: string }>(
    'background-effect-changed',
    (event) => {
      const effect = event.payload?.effect
      if (effect) {
        uiStore.setBackgroundEffect(effect)
      }
    }
  )

  volumeUnlisten = await appWindow.listen<{ volume: number }>(
    'pet-volume-changed',
    (event) => {
      const volume = Number(event.payload?.volume)
      if (!Number.isNaN(volume)) {
        settingsStore.updateAudio({ characterVolume: volume })
      }
    }
  )
  // 设置透明背景的 body 属性样式（额外防护）
  document.body.style.backgroundColor = 'transparent'
  document.documentElement.style.backgroundColor = 'transparent'

  // 1. 初始化窗口为桌宠尺寸
  await applyWindowLayout()

  // 2. 启动 100ms 一次的 solid bounds 测试
  hitTestInterval = window.setInterval(() => {
    const rects = []
    
    // 如果对话气泡正在显示，则加入 solid region 触发交互
    if (
      dialogContainer.value &&
      gameStore.currentStatus === 'responding' &&
      gameStore.currentLine.trim() !== ''
    ) {
      const r = dialogContainer.value.getBoundingClientRect()
      rects.push({ x: r.x, y: r.y, width: r.width, height: r.height })
    }
    
    // 头像圆环常驻 solid region 触发拖拽和交互
    if (avatarContainer.value) {
      const r = avatarContainer.value.getBoundingClientRect()
      rects.push({ x: r.x, y: r.y, width: r.width, height: r.height })
    }
    
    // 输入框显示时，加入 solid region
    if (chatContainer.value && showChatInput.value) {
      const r = chatContainer.value.getBoundingClientRect()
      // 输入框稍微拓宽，保证极小尺寸下的鼠标判定连贯性
      rects.push({
        x: r.x - 20,
        y: r.y - 20,
        width: r.width + 40,
        height: r.height + 40,
      })
    }
    
    invoke('update_solid_regions', { rects }).catch(console.error)
  }, 100)
})

watch(
  () => settingsStore.pet?.scale,
  () => {
    void applyWindowLayout()
  },
)

onUnmounted(() => {
  // 恢复默认背景色
  document.body.style.backgroundColor = ''
  document.documentElement.style.backgroundColor = ''

  if (scaleUnlisten) scaleUnlisten()
  if (effectUnlisten) effectUnlisten()
  if (volumeUnlisten) volumeUnlisten()

  if (hitTestInterval !== undefined) {
    window.clearInterval(hitTestInterval)
  }
})

const handleMessageSent = (message: string) => {
  gameStore.appendGameMessage({
    type: 'message',
    displayName: gameStore.userName,
    content: message,
  })
}

const handleMouseEnter = () => {
  showChatInput.value = true
}

const handleMouseLeave = () => {
  showChatInput.value = false
}

const handleAvatarClick = () => {
  manualTriggerContinue()
  eventQueue.continue()
  resetInteraction()
}

const handleOpenSettings = async () => {
  try {
    const existing = await WebviewWindow.getByLabel('settings')
    if (existing) {
      await existing.setFocus()
      return
    }

    const webview = new WebviewWindow('settings', {
      url: '/second',
      title: '设置',
      width: 1200,
      height: 800,
      resizable: true,
      shadow: false,
      decorations: false,
      transparent: true,
      alwaysOnTop: false,
    })

    webview.once('tauri://created', () => {
      console.log('桌宠轻量设置窗口创建成功')
    })

    webview.once('tauri://error', (e) => {
      console.error('创建桌宠轻量设置窗口失败:', e)
    })
  } catch (error) {
    console.error('打开设置窗口时出错:', error)
  }
}

// 自动打字/对话逻辑
let timerId: any = null
const isContinueTriggered = ref(false)
const audioFinished = ref(true)

const resetInteraction = () => {
  isContinueTriggered.value = false
  audioFinished.value = true
  if (timerId) {
    clearTimeout(timerId)
    timerId = null
  }
}

const tryAutoAdvance = () => {
  if (!uiStore.autoMode) return
  if (isContinueTriggered.value) return
  if (gameStore.currentStatus !== 'responding') return

  const typing = gameDialogRef.value?.isTyping ?? false
  if (typing || !audioFinished.value) return

  if (timerId) clearTimeout(timerId)
  timerId = setTimeout(() => {
    if (gameDialogRef.value) {
      const needWait = gameDialogRef.value.continueDialog(false)
      if (needWait) {
        tryAutoAdvance()
      }
    }
  }, 1000)
}

const handleAudioStarted = () => {
  audioFinished.value = false
}

const handleAudioFinished = () => {
  audioFinished.value = true
  tryAutoAdvance()
}

watch(
  () => gameDialogRef.value?.isTyping,
  (typing) => {
    if (typing === false) {
      tryAutoAdvance()
    }
  },
)

const manualTriggerContinue = () => {
  if (timerId) {
    clearTimeout(timerId)
    timerId = null
  }
  if (!isContinueTriggered.value) {
    isContinueTriggered.value = true
  }
}

const handleSwitchAutoMode = () => {
  uiStore.autoMode = !uiStore.autoMode
}

const handleExitPetMode = async () => {
  // 退出时清除 solid region，防止残留
  await invoke('update_solid_regions', { rects: [] })
  // 1. 关闭桌宠窗口特性，恢复 1500x800 的正常主窗口
  await invoke('set_pet_mode', { enable: false })
  // 2. 路由导航回聊天主页面
  router.push('/chat')
}
</script>

<style scoped>
#pet-app {
  position: relative;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
}
</style>

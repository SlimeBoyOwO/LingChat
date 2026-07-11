<template>
  <div
    id="pet-app"
    :style="appStyleVars"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    class="relative w-(--app-width) h-(--app-height) flex flex-col justify-start items-center overflow-hidden transition-none select-none bg-transparent"
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
        v-if="windowReady"
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
import { onBeforeRouteLeave, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { currentMonitor, getCurrentWindow } from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { useGameStore } from '@/stores/modules/game'
import { useSettingsStore } from '@/stores/modules/settings'
import { useUIStore } from '@/stores/modules/ui/ui'
import { eventQueue } from '@/core/events/event-queue'
import {
  invalidatePetWindowModePreparation,
  isPetWindowModePrepared,
  normalizePetScale,
  setPetWindowModeAndWait,
} from '@/utils/windowSizing'

import ChatInput from '../pet/ChatInput.vue'
import DialogueBox from '../pet/DialogueBox.vue'
import GameRolesStage from '../pet/GameRolesStage.vue'
import { setExternalDeliveryBlocked } from '@/composables/useCanDeliver'

const BASE_AVATAR_SIZE = 240
const CHAT_BASE_H = 45
const DIALOG_BASE_H = 75
const SETTINGS_TARGET_WIDTH = 1200
const SETTINGS_TARGET_HEIGHT = 800
const SETTINGS_MIN_WIDTH = 720
const SETTINGS_MIN_HEIGHT = 480
const SETTINGS_WORK_AREA_RATIO = 0.9

const router = useRouter()
const gameStore = useGameStore()
const settingsStore = useSettingsStore()
const uiStore = useUIStore()

const showChatInput = ref(false)
const windowReady = ref(false)

const dialogContainer = ref<HTMLElement | null>(null)
const avatarContainer = ref<HTMLElement | null>(null)
const chatContainer = ref<HTMLElement | null>(null)
const gameDialogRef = ref<InstanceType<typeof DialogueBox> | null>(null)

const appStyleVars = computed(() => {
  const scale = normalizePetScale(settingsStore.pet?.scale)
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

const applyWindowLayout = (scale = normalizePetScale(settingsStore.pet?.scale)) => {
  return setPetWindowModeAndWait(true, scale)
}

let hitTestInterval: number | undefined
let scaleUnlisten: (() => void) | null = null
let scaleFactorUnlisten: (() => void) | null = null
let effectUnlisten: (() => void) | null = null
let volumeUnlisten: (() => void) | null = null
let dialogHistoryUnlisten: (() => void) | null = null
let scaleApplyTimer: number | null = null
let scaleApplyRevision = 0
let disposed = false
let exitingPetMode = false
let mainWindowRestored = false
let restorePromise: Promise<void> | null = null
let exitNavigationInFlight = false

const isPetModeInactive = () => disposed || exitingPetMode

const showPetWindowError = (title: string, error: unknown) => {
  const message = error instanceof Error ? error.message : String(error || '未知错误')
  uiStore.showNotification({
    type: 'error',
    title,
    message,
    duration: 5000,
    skipTipsCheck: true,
  })
}

const cancelScheduledWindowLayout = () => {
  scaleApplyRevision += 1
  if (scaleApplyTimer !== null) {
    window.clearTimeout(scaleApplyTimer)
    scaleApplyTimer = null
  }
}

const scheduleWindowLayout = (scale: number) => {
  if (isPetModeInactive()) return

  const revision = ++scaleApplyRevision
  if (scaleApplyTimer !== null) window.clearTimeout(scaleApplyTimer)

  scaleApplyTimer = window.setTimeout(async () => {
    scaleApplyTimer = null
    if (isPetModeInactive() || revision !== scaleApplyRevision) return

    try {
      const isLatestRequest = await applyWindowLayout(scale)
      if (!isPetModeInactive() && revision === scaleApplyRevision && isLatestRequest) {
        windowReady.value = true
      }
    } catch (error) {
      if (isPetModeInactive() || revision !== scaleApplyRevision) return
      windowReady.value = true
      console.error('调整桌宠窗口布局失败:', error)
      showPetWindowError('桌宠缩放失败', error)
    }
  }, 100)
}

const closeSettingsWindow = async () => {
  try {
    const settingsWindow = await WebviewWindow.getByLabel('settings')
    if (settingsWindow) await settingsWindow.close()
  } catch (error) {
    console.warn('关闭桌宠设置窗口失败:', error)
  } finally {
    setExternalDeliveryBlocked(false)
  }
}

/** Restores the main window once, sharing an in-flight restore across all exit paths. */
const restoreMainWindow = (): Promise<void> => {
  if (mainWindowRestored) return Promise.resolve()
  if (restorePromise) return restorePromise

  exitingPetMode = true
  windowReady.value = false
  cancelScheduledWindowLayout()

  restorePromise = (async () => {
    await closeSettingsWindow()
    await invoke('update_solid_regions', { rects: [] }).catch((error) => {
      console.warn('清除桌宠交互区域失败:', error)
    })

    const isLatestRequest = await setPetWindowModeAndWait(false)
    if (!isLatestRequest) {
      throw new Error('恢复主窗口的请求已被更新的窗口操作取代')
    }

    invalidatePetWindowModePreparation()
    mainWindowRestored = true
  })().catch((error) => {
    restorePromise = null
    exitingPetMode = false
    if (!disposed) windowReady.value = true
    throw error
  })

  return restorePromise
}

onBeforeRouteLeave(async () => {
  try {
    // Button-driven navigation reaches this guard after the same promise has resolved,
    // while browser-back and redirects perform the restore here.
    await restoreMainWindow()
    return true
  } catch (error) {
    console.error('离开桌宠页面前恢复主窗口失败:', error)
    showPetWindowError('退出桌宠模式失败', error)
    return false
  }
})

const retainUnlisten = (
  unlisten: (() => void) | null,
  assign: (value: (() => void) | null) => void,
): boolean => {
  if (isPetModeInactive()) {
    unlisten?.()
    return false
  }
  assign(unlisten)
  return true
}

onMounted(async () => {
  const appWindow = getCurrentWindow()

  // 设置透明背景，并先确认窗口已经稳定；正常入口复用 MainChat 已完成的切换，
  // 直达 /pet 时才补做初始化，避免路由前后重复 set_size。
  document.body.style.backgroundColor = 'transparent'
  document.documentElement.style.backgroundColor = 'transparent'
  const initialScale = normalizePetScale(settingsStore.pet?.scale)
  if (isPetWindowModePrepared(initialScale)) {
    windowReady.value = true
  } else {
    try {
      const isLatestRequest = await applyWindowLayout(initialScale)
      if (isPetModeInactive()) return
      if (!isLatestRequest) throw new Error('初始化桌宠窗口的请求已失效')
      windowReady.value = true
    } catch (error) {
      if (isPetModeInactive()) return
      console.error('初始化桌宠窗口失败:', error)
      showPetWindowError('进入桌宠模式失败', error)
      try {
        await restoreMainWindow()
        if (!disposed) await router.replace('/chat')
      } catch (restoreError) {
        console.error('初始化失败后恢复主窗口失败:', restoreError)
        showPetWindowError('恢复主窗口失败', restoreError)
      }
      return
    }
  }

  if (isPetModeInactive()) return

  const pendingScaleUnlisten = await appWindow
    .listen<{ scale: number }>('pet-scale-changed', (event) => {
      const rawScale = Number(event.payload?.scale)
      if (!isPetModeInactive() && Number.isFinite(rawScale)) {
        const scale = normalizePetScale(rawScale)
        settingsStore.pet.scale = scale
        scheduleWindowLayout(scale)
      }
    })
    .catch((error) => {
      console.error('监听桌宠缩放失败:', error)
      return null
    })
  if (!retainUnlisten(pendingScaleUnlisten, (value) => (scaleUnlisten = value))) return

  const pendingScaleFactorUnlisten = await appWindow
    .onScaleChanged(() => {
      if (isPetModeInactive()) return
      invalidatePetWindowModePreparation()
      scheduleWindowLayout(normalizePetScale(settingsStore.pet?.scale))
    })
    .catch((error) => {
      console.error('监听窗口 DPI 变化失败:', error)
      return null
    })
  if (!retainUnlisten(pendingScaleFactorUnlisten, (value) => (scaleFactorUnlisten = value))) return

  const pendingEffectUnlisten = await appWindow
    .listen<{ effect: string }>('background-effect-changed', (event) => {
      if (isPetModeInactive()) return
      const effect = event.payload?.effect
      if (effect) {
        uiStore.setBackgroundEffect(effect)
      }
    })
    .catch((error) => {
      console.error('监听桌宠背景特效失败:', error)
      return null
    })
  if (!retainUnlisten(pendingEffectUnlisten, (value) => (effectUnlisten = value))) return

  const pendingVolumeUnlisten = await appWindow
    .listen<{ volume: number }>('pet-volume-changed', (event) => {
      if (isPetModeInactive()) return
      const volume = Number(event.payload?.volume)
      if (!Number.isNaN(volume)) {
        settingsStore.updateAudio({ characterVolume: volume })
      }
    })
    .catch((error) => {
      console.error('监听桌宠音量失败:', error)
      return null
    })
  if (!retainUnlisten(pendingVolumeUnlisten, (value) => (volumeUnlisten = value))) return

  // 响应设置窗口的初始历史数据请求
  const pendingDialogHistoryUnlisten = await appWindow
    .listen('request-dialog-history', () => {
      if (isPetModeInactive()) return
      appWindow.emit('dialog-history-changed', {
        dialogHistory: JSON.parse(JSON.stringify(gameStore.dialogHistory)),
      })
    })
    .catch((error) => {
      console.error('监听对话历史请求失败:', error)
      return null
    })
  if (!retainUnlisten(pendingDialogHistoryUnlisten, (value) => (dialogHistoryUnlisten = value)))
    return

  // 2. 启动 100ms 一次的 solid bounds 测试
  hitTestInterval = window.setInterval(() => {
    if (isPetModeInactive()) return
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

// 监听 dialogHistory 变化，推送给设置窗口
watch(
  () => gameStore.dialogHistory.length,
  () => {
    if (isPetModeInactive()) return
    const appWindow = getCurrentWindow()
    appWindow.emit('dialog-history-changed', {
      dialogHistory: JSON.parse(JSON.stringify(gameStore.dialogHistory)),
    })
  },
)

onUnmounted(() => {
  disposed = true
  invalidatePetWindowModePreparation()
  cancelScheduledWindowLayout()

  // 恢复默认背景色
  document.body.style.backgroundColor = ''
  document.documentElement.style.backgroundColor = ''

  if (scaleUnlisten) scaleUnlisten()
  if (scaleFactorUnlisten) scaleFactorUnlisten()
  if (effectUnlisten) effectUnlisten()
  if (volumeUnlisten) volumeUnlisten()
  if (dialogHistoryUnlisten) dialogHistoryUnlisten()

  if (hitTestInterval !== undefined) {
    window.clearInterval(hitTestInterval)
    hitTestInterval = undefined
  }

  if (timerId !== null) {
    window.clearTimeout(timerId)
    timerId = null
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

const getSettingsWindowBounds = async () => {
  const monitor = await currentMonitor().catch((error) => {
    console.warn('读取当前显示器工作区失败，使用默认设置窗口尺寸:', error)
    return null
  })

  if (!monitor || !Number.isFinite(monitor.scaleFactor) || monitor.scaleFactor <= 0) {
    return {
      width: SETTINGS_TARGET_WIDTH,
      height: SETTINGS_TARGET_HEIGHT,
      minWidth: SETTINGS_MIN_WIDTH,
      minHeight: SETTINGS_MIN_HEIGHT,
      maxWidth: SETTINGS_TARGET_WIDTH,
      maxHeight: SETTINGS_TARGET_HEIGHT,
    }
  }

  const logicalWorkWidth = monitor.workArea.size.width / monitor.scaleFactor
  const logicalWorkHeight = monitor.workArea.size.height / monitor.scaleFactor
  const maxWidth = Math.max(1, Math.floor(logicalWorkWidth * SETTINGS_WORK_AREA_RATIO))
  const maxHeight = Math.max(1, Math.floor(logicalWorkHeight * SETTINGS_WORK_AREA_RATIO))
  const width = Math.min(SETTINGS_TARGET_WIDTH, maxWidth)
  const height = Math.min(SETTINGS_TARGET_HEIGHT, maxHeight)

  return {
    width,
    height,
    minWidth: Math.min(SETTINGS_MIN_WIDTH, width),
    minHeight: Math.min(SETTINGS_MIN_HEIGHT, height),
    maxWidth,
    maxHeight,
  }
}

const handleOpenSettings = async () => {
  if (isPetModeInactive()) return

  setExternalDeliveryBlocked(true)
  try {
    const existing = await WebviewWindow.getByLabel('settings')
    if (isPetModeInactive()) {
      setExternalDeliveryBlocked(false)
      return
    }

    if (existing) {
      void existing.once('tauri://destroyed', () => {
        setExternalDeliveryBlocked(false)
      })
      await existing.setFocus()
      return
    }

    const bounds = await getSettingsWindowBounds()
    if (isPetModeInactive()) {
      setExternalDeliveryBlocked(false)
      return
    }

    const webview = new WebviewWindow('settings', {
      url: '/second',
      title: '设置',
      ...bounds,
      center: true,
      preventOverflow: true,
      resizable: true,
      shadow: false,
      decorations: false,
      transparent: true,
      alwaysOnTop: false,
    })

    webview.once('tauri://created', () => {
      if (isPetModeInactive()) {
        void webview.close()
        return
      }
      void webview.center().catch((error) => {
        console.warn('居中桌宠设置窗口失败:', error)
      })
      console.log('桌宠轻量设置窗口创建成功')
    })

    webview.once('tauri://error', (e) => {
      setExternalDeliveryBlocked(false)
      console.error('创建桌宠轻量设置窗口失败:', e)
    })
    webview.once('tauri://destroyed', () => {
      setExternalDeliveryBlocked(false)
    })
  } catch (error) {
    setExternalDeliveryBlocked(false)
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
  if (exitNavigationInFlight) return
  exitNavigationInFlight = true

  try {
    await restoreMainWindow()
    // The leave guard reuses the resolved restore promise instead of invoking Tauri twice.
    await router.push('/chat')
  } catch (error) {
    console.error('退出桌宠模式失败:', error)
    showPetWindowError('退出桌宠模式失败', error)
  } finally {
    exitNavigationInFlight = false
  }
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

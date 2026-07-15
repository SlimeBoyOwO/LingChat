<template>
  <router-view />
  <CursorEffects />

  <!-- 全局通知组件（直接从 uiStore 读取状态） -->
  <Notification />
  <AchievementToast />
  <AdventureUnlockNotify />
  <AppDialog />
</template>

<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import CursorEffects from './components/effects/CursorEffects.vue'
import Notification from './components/ui/Notification.vue'
import AchievementToast from './components/ui/AchievementToast.vue'
import AdventureUnlockNotify from './components/ui/AdventureUnlockNotify.vue'
import AppDialog from './components/ui/AppDialog.vue'
import { initUIStore } from './stores/modules/ui/ui'
import { useLlmProvidersStore } from './stores/modules/llm-providers'
import { useAchievementStore } from './stores/modules/ui/achievement'
import { useDialogStore } from './stores/modules/ui/dialog'
import { useSedentaryReminder } from './composables/useSedentaryReminder'
import { useUpdater } from './composables/useUpdater'
import { useCanDeliver } from './composables/useCanDeliver'

// 激活主动对话投放条件上报（仅在此处挂载一次）
useCanDeliver()

// ─── 久坐提醒 ────────────────────────────────────────────────
useSedentaryReminder()

// ─── 键盘处理 ────────────────────────────────────────────────

const route = useRoute()

const handleKeyDown = async (event: KeyboardEvent) => {
  if (event.key === 'F11') {
    event.preventDefault()

    // Pet 路由时不允许全屏
    if (route.path === '/pet') {
      return
    }

    try {
      const appWindow = getCurrentWindow()
      const isFullscreen = await appWindow.isFullscreen()
      await appWindow.setFullscreen(!isFullscreen)
    } catch (e) {
      console.error('全屏切换失败:', e)
    }
  }
}

// ─── 关闭确认 ────────────────────────────────────────────────

const dialogStore = useDialogStore()
let saveCompleted = false
let userConfirmedExit = false
let unlistenCloseReady: (() => void) | null = null
let unlistenCloseRequested: (() => void) | null = null

// 处理退出：两个条件都满足时调用 Rust exit_app
function tryExit() {
  if (saveCompleted && userConfirmedExit) {
    invoke('exit_app')
  }
}

onMounted(async () => {
  // 初始化 UI Store（加载角色 tips）
  initUIStore()

  // 预加载 LLM 提供商配置，避免主界面因 store 未加载而误判未选择模型
  const llmStore = useLlmProvidersStore()
  llmStore.load().catch((e) => console.error('加载 LLM 提供商失败:', e))

  // 供成就系统控制台测试用，在 window 对象中注册一些方法
  const achievementStore = useAchievementStore()
  ;(window as any).requestAchievementUnlock = (data: any) => achievementStore.notifyBackendUnlock(data)
  ;(window as any).showAchievement = (data: any) => achievementStore.addAchievement(data)
  // 成就系统启动WebSocket监听
  achievementStore.listenForUnlocks()

  // 注册 F11 全屏快捷键
  window.addEventListener('keydown', handleKeyDown)

  // ─── 关闭确认逻辑 ──────────────────────────────────────────

  // 1. 监听 Rust 存档完成事件
  unlistenCloseReady = await listen('app:close-ready', () => {
    saveCompleted = true
    tryExit()
  })

  // 2. 拦截窗口关闭请求
  unlistenCloseRequested = await getCurrentWindow().onCloseRequested(async (event: { preventDefault: () => void }) => {
    event.preventDefault()

    // 重置状态
    saveCompleted = false
    userConfirmedExit = false

    if (route.path === '/chat') {
      const confirmed = await dialogStore.confirm('确定要退出程序吗？', '退出确认')
      if (!confirmed) return // 用户取消，窗口保持打开
    }

    userConfirmedExit = true
    tryExit()
  })
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown)
  if (unlistenCloseReady) unlistenCloseReady()
  if (unlistenCloseRequested) unlistenCloseRequested()
})
</script>

<style>
:root {
  /*全局变量*/
  --accent-color: #79d9ff;
  --menu-max-width: 1100px;
  --menu-max-width-half: 550px;
  /* 一个生动的天蓝色，可以根据你的品牌调整 */
}

/* 全局样式和字体 */
body,
html {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  font-family:
    -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  overflow: hidden;
  background: transparent;
  /* 确保body背景透明，不遮挡我们的背景图 */
}

#app {
  width: 100vw;
  height: 100vh;
}
</style>

<template>
  <!-- 全局启动加载盖层 -->
  <Loader :loading="uiStore.appLoading" :progress="loadingProgress" />

  <router-view />
  <CursorEffects />

  <!-- 全局通知组件（直接从 uiStore 读取状态） -->
  <Notification />
  <AchievementToast />
  <AdventureUnlockNotify />

  <!-- 新手引导教程遮罩 -->
  <TutorialOverlay />
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { connectionReady } from './api/websocket'
import { useUIStore, initUIStore } from './stores/modules/ui/ui'
import CursorEffects from './components/effects/CursorEffects.vue'
import Notification from './components/ui/Notification.vue'
import AchievementToast from './components/ui/AchievementToast.vue'
import AdventureUnlockNotify from './components/ui/AdventureUnlockNotify.vue'
import TutorialOverlay from './components/ui/TutorialOverlay.vue'
import { useAchievementStore } from './stores/modules/ui/achievement'
import { useTutorialStore } from './stores/modules/tutorial'

const uiStore = useUIStore()

// Loader 进度条（0~100，模拟动画）
const loadingProgress = ref(0)
let progressTimer = null

// 启动进度条动画（匀速增长到 85%，最后 15% 留给完成瞬间）
function startProgressAnimation() {
  progressTimer = setInterval(() => {
    if (loadingProgress.value < 85) {
      // 先快后慢，更自然
      const step = Math.max(0.5, (85 - loadingProgress.value) * 0.04)
      loadingProgress.value = Math.min(85, loadingProgress.value + step)
    }
  }, 200)
}

function stopProgressAnimation() {
  if (progressTimer) {
    clearInterval(progressTimer)
    progressTimer = null
  }
}

// 完成加载：进度拉满后关闭
function finishLoading() {
  stopProgressAnimation()
  loadingProgress.value = 100
  // 等进度条动画渲染到 100%
  setTimeout(() => {
    uiStore.completeAppLoading()
  }, 400)
}

onMounted(async () => {
  // 初始化 UI Store（加载角色 tips）
  initUIStore()

  // 启动 Loader 进度条动画
  startProgressAnimation()

  // 记录开始时间，确保最短展示时长
  const startTime = Date.now()
  const MIN_DURATION = 1500
  const WS_TIMEOUT = 3000

  // 条件①：等待首屏子组件挂载（nextTick 后 router-view 已渲染）
  await nextTick()

  // 条件②：WS 连接成功 或 超时（无后端开发也能进入界面）
  await Promise.race([
    // WS 连接成功
    new Promise((resolve) => {
      if (connectionReady.value) {
        resolve()
      } else {
        const unwatch = watch(connectionReady, (val) => {
          if (val) {
            unwatch()
            resolve()
          }
        })
      }
    }),
    // 超时
    new Promise((resolve) => setTimeout(resolve, WS_TIMEOUT)),
  ])

  // 条件③：保证最短展示时长，防止闪烁
  const elapsed = Date.now() - startTime
  if (elapsed < MIN_DURATION) {
    await new Promise((r) => setTimeout(r, MIN_DURATION - elapsed))
  }

  // ---------- 关闭 Loader 前：处理新手教程（让教程在 Loader 下方预渲染） ----------
  // checkBackendConfig() 内部已处理两种模式：
  //   global 模式：读取后端 SHOW_ONBOARDING_TUTORIAL 决定
  //   per-device 模式：读取本地 isCompleted 决定
  const tutorialStore = useTutorialStore()
  let tutorialFirstFrameWaiter: Promise<void> | null = null

  try {
    const shouldShow = await tutorialStore.checkBackendConfig()
    if (shouldShow) {
      tutorialStore.startPreset('onboarding')
      // 等待教程第一帧渲染完成（Loader 关闭后教程立即可见，无闪白间隙）
      tutorialFirstFrameWaiter = tutorialStore.waitForFirstFrame()
    }
  } catch (e) {
    console.warn('[Tutorial] 检查配置失败，跳过新手引导', e)
  }

  // 关闭 Loader —— 等待教程第一帧（如果有）或立即关闭
  // 设置 3s 超时防止教程卡死阻塞主界面
  if (tutorialFirstFrameWaiter) {
    await Promise.race([tutorialFirstFrameWaiter, new Promise((r) => setTimeout(r, 3000))])
  }
  finishLoading()

  // ---------- 以下是非阻塞的后续初始化 ----------

  // 供成就系统控制台测试用
  const achievementStore = useAchievementStore()
  window.requestAchievementUnlock = (data) => achievementStore.notifyBackendUnlock(data)
  window.showAchievement = (data) => achievementStore.addAchievement(data)
  // 成就系统启动WebSocket监听
  achievementStore.listenForUnlocks()

  // 等待 pywebview API 准备就绪
  window.addEventListener('pywebviewready', () => {
    window.addEventListener('keydown', handleKeyDown)
  })
})

const handleKeyDown = (event) => {
  if (event.key === 'F11') {
    event.preventDefault()
    if (
      window.pywebview &&
      window.pywebview.api &&
      typeof window.pywebview.api.toggle_fullscreen === 'function'
    ) {
      window.pywebview.api.toggle_fullscreen()
    } else {
      console.error('全屏API不可用。')
    }
  }
}

onUnmounted(() => {
  stopProgressAnimation()
  window.removeEventListener('keydown', handleKeyDown)
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

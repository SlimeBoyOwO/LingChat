<template>
  <div class="main-box">
    <!-- 主界面始终渲染，加载动画期间在后台初始化 -->
    <FreeModeTools />
    <GameBackground></GameBackground>
    <!-- <GameAvatar ref="gameAvatarRef" @audio-ended="handleAudioFinished" />  -->
    <GameRolesStage
      ref="gameAvatarRef"
      @audio-ended="handleAudioFinished"
      @audio-started="handleAudioStarted"
    />
    <GameDialog
      ref="gameDialogRef"
      @player-continued="manualTriggerContinue"
    />

    <!-- 原有的菜单按钮 -->
    <div id="menu-panel">
      <Button
        type="nav"
        icon="play"
        @click="switchAutoMode"
        :active="uiStore.autoMode"
        v-show="uiStore.showSettings !== true"
      >
        <h3 class="hidden xl:block">自动</h3>
      </Button>
      <Button
        type="nav"
        icon="character"
        @click="goToPetMode"
        v-show="uiStore.showSettings !== true"
      >
        <h3 class="hidden xl:block">桌宠</h3>
      </Button>
      <Button type="nav" icon="text" @click="openSettings" v-show="uiStore.showSettings !== true">
        <h3 class="hidden xl:block">菜单</h3>
      </Button>
    </div>
    <GameExtraUI />

    <!-- 首次加载过渡动画（覆盖在主界面上方，主界面在后台并行初始化） -->
    <LoadingTransition v-if="showLoading" @complete="onLoadingComplete" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import FreeModeTools from '@/components/tools/FreeModeTools.vue'
import { useUIStore } from '../../stores/modules/ui/ui'
import { useGameStore } from '../../stores/modules/game'
import { GameBackground, GameRolesStage } from '../game/standard'
import { GameDialog } from '../game/standard'
import { Button } from '../base'
import LoadingTransition from './LoadingTransition.vue'
import { eventQueue } from '@/core/events/event-queue'

import GameExtraUI from '../game/standard/GameExtraUI.vue'

const LOADING_STORAGE_KEY = 'lingchat_loading_shown'

const router = useRouter()
const uiStore = useUIStore()
const gameStore = useGameStore()

// 首次加载过渡状态（通过 localStorage 跨路由导航保持，启动时由 main.ts 清除）
const showLoading = ref(!localStorage.getItem(LOADING_STORAGE_KEY))

function onLoadingComplete() {
  showLoading.value = false
  localStorage.setItem(LOADING_STORAGE_KEY, '1')
  // 加载动画结束，恢复事件队列消费
  eventQueue.resume()
}

const goToPetMode = () => {
  router.push('/pet')
}

const gameDialogRef = ref<InstanceType<typeof GameDialog> | null>(null)

const openSettings = () => {
  // 后台截图（不阻塞 UI），设置面板立即打开
  gameStore.captureScreenshot()
  uiStore.toggleSettings(true)
  uiStore.setSettingsTab('text')
}

const switchAutoMode = () => {
  uiStore.autoMode = !uiStore.autoMode
}

const runInitialization = async () => {
  try {
    await gameStore.initializeGame()
  } catch (error) {
    console.log(error)
  }
}

// 初始化游戏信息
onMounted(() => {
  if (!gameStore.initialized) {
    runInitialization()
  }
})

/* 自动模式（AUTO）逻辑：事件驱动，非轮询
 * 当且仅当以下全部满足时，延迟 1 秒自动推进下一句：
 * 1. 自动模式开启
 * 2. 当前处于 responding 状态
 * 3. 当前台词打字机已结束
 * 4. 当前台词语音已播放完毕
 * 用户手动推进时取消当前调度。
 */
const AUTO_ADVANCE_DELAY_MS = 1000

const typingFinished = ref(true)
const audioFinished = ref(true)
let autoAdvanceTimer: ReturnType<typeof setTimeout> | null = null

const cancelAutoAdvance = () => {
  if (autoAdvanceTimer) {
    clearTimeout(autoAdvanceTimer)
    autoAdvanceTimer = null
  }
}

const scheduleAutoAdvance = () => {
  cancelAutoAdvance()

  if (!uiStore.autoMode) return
  if (gameStore.currentStatus !== 'responding') return
  if (!typingFinished.value || !audioFinished.value) return

  autoAdvanceTimer = setTimeout(() => {
    autoAdvanceTimer = null
    if (!uiStore.autoMode || gameStore.currentStatus !== 'responding') return
    if (!typingFinished.value || !audioFinished.value) return

    const needWait = gameDialogRef.value?.continueDialog(false) ?? true
    if (!needWait) {
      // 推进后重置状态，等待下一条台词的打字/语音事件
      typingFinished.value = true
      audioFinished.value = true
    }
  }, AUTO_ADVANCE_DELAY_MS)
}

// 音频开始播放
const handleAudioStarted = () => {
  audioFinished.value = false
  cancelAutoAdvance()
}

// 音频播放结束
const handleAudioFinished = () => {
  audioFinished.value = true
  scheduleAutoAdvance()
}

// 用户手动推进
const manualTriggerContinue = () => {
  cancelAutoAdvance()
}

// 监听自动模式开关
watch(
  () => uiStore.autoMode,
  (enabled) => {
    if (enabled) scheduleAutoAdvance()
    else cancelAutoAdvance()
  },
)

// 监听游戏状态：进入 responding 时重置状态并等待事件
watch(
  () => gameStore.currentStatus,
  (status) => {
    if (status === 'responding') {
      typingFinished.value = !(gameDialogRef.value?.isTyping ?? false)
      audioFinished.value = true // 新台词初始无音频
      scheduleAutoAdvance()
    } else {
      cancelAutoAdvance()
    }
  },
)

// 监听打字状态：结束立即尝试推进，开始则取消
watch(
  () => gameDialogRef.value?.isTyping,
  (typing) => {
    if (typing) {
      typingFinished.value = false
      cancelAutoAdvance()
    } else {
      typingFinished.value = true
      scheduleAutoAdvance()
    }
  },
)
</script>

<style>
.main-box {
  position: absolute;
  height: 100%;
  width: 100%;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  align-items: center;
  overflow: hidden;
}

#menu-panel {
  display: flex;
  position: fixed;
  top: 15px;
  right: 20px;
  z-index: 1000;
}
.scene-controls {
  position: fixed;
  bottom: 80px; /* 根据聊天输入框高度调整 */
  left: 20px;
  display: flex;
  gap: 8px;
  align-items: center;
  background: rgba(0, 0, 0, 0.5);
  padding: 8px 12px;
  border-radius: 20px;
  backdrop-filter: blur(5px);
  z-index: 100;
}

.scene-indicator {
  color: #fff;
  font-size: 14px;
  margin-left: 8px;
}
</style>

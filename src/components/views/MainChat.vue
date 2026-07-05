<template>
  <div class="main-box">
    <!-- 左上角番茄钟开关与面板 -->
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
      @dialog-proceed="resetInteraction"
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

import GameExtraUI from '../game/standard/GameExtraUI.vue'

const router = useRouter()
const uiStore = useUIStore()
const gameStore = useGameStore()

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

/* 以下代码为自动AUTO模式逻辑 比较复杂 */
// 1. 用于存储 setTimeout 返回的 ID
let timerId: any = null
// 2. 状态标志，记录 continue() 是否已被调用
const isContinueTriggered = ref(false)
// 3. 追踪音频和打字状态
const audioFinished = ref(true) // 默认 true（无音频时视为已完成）
// 4. 轮询检查间隔（毫秒）
const AUTO_ADVANCE_INTERVAL = 500

// 在新交互开始前调用的重置函数
const resetInteraction = () => {
  isContinueTriggered.value = false
  audioFinished.value = true
  if (timerId) {
    clearTimeout(timerId)
    timerId = null
  }
}

// 清理当前轮询（用于关闭自动模式或组件卸载）
const clearAutoAdvance = () => {
  if (timerId) {
    clearTimeout(timerId)
    timerId = null
  }
}

// 尝试触发自动继续（打字和音频都结束后才执行）
const tryAutoAdvance = () => {
  // 自动模式关闭时立即停止
  if (!uiStore.autoMode) {
    clearAutoAdvance()
    return
  }

  // 已经触发过继续，或当前不在回应状态，都只做一次清理后退出
  if (isContinueTriggered.value) {
    clearAutoAdvance()
    return
  }

  if (gameStore.currentStatus !== 'responding') {
    // 状态不对时延迟重试，避免状态切换期间漏触发
    scheduleNextCheck()
    return
  }

  const typing = gameDialogRef.value?.isTyping ?? false
  if (typing || !audioFinished.value) {
    // 条件未满足，继续轮询等待
    scheduleNextCheck()
    return
  }

  // 条件满足：延迟 1 秒后触发继续
  clearAutoAdvance()
  timerId = setTimeout(() => {
    timerId = null
    if (!uiStore.autoMode || isContinueTriggered.value) return
    if (gameDialogRef.value) {
      const needWait = gameDialogRef.value.continueDialog(false)
      if (needWait) {
        // 还需要等待，继续轮询
        scheduleNextCheck()
      }
    }
  }, 1000)
}

// 安排下一次轮询检查
const scheduleNextCheck = () => {
  clearAutoAdvance()
  timerId = setTimeout(() => {
    timerId = null
    tryAutoAdvance()
  }, AUTO_ADVANCE_INTERVAL)
}

// 音频开始播放
const handleAudioStarted = () => {
  audioFinished.value = false
  tryAutoAdvance()
}

// 音频播放结束
const handleAudioFinished = () => {
  audioFinished.value = true
  tryAutoAdvance()
}

// 监听打字结束
watch(
  () => gameDialogRef.value?.isTyping,
  (typing) => {
    if (typing === false) {
      tryAutoAdvance()
    }
  },
)

// 监听自动模式开关：开启时立即启动轮询，关闭时清理
watch(
  () => uiStore.autoMode,
  (enabled) => {
    if (enabled) {
      tryAutoAdvance()
    } else {
      clearAutoAdvance()
    }
  },
)

// 监听游戏状态：进入 responding 时重置继续标志，让自动模式可以工作
watch(
  () => gameStore.currentStatus,
  (status) => {
    if (status === 'responding') {
      // 新回应开始时，允许自动继续
      isContinueTriggered.value = false
      audioFinished.value = true
      tryAutoAdvance()
    } else if (status === 'input') {
      // 进入输入状态时清理轮询，避免无意义等待
      clearAutoAdvance()
    }
  },
)

// 用户手动触发的函数
const manualTriggerContinue = () => {
  clearAutoAdvance()

  if (!isContinueTriggered.value) {
    isContinueTriggered.value = true
  }
}
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

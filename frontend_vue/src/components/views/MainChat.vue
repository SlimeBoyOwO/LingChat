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
        :class="[{ active: uiStore.autoMode }]"
        v-show="uiStore.showSettings !== true"
      >
        <h3>自动</h3>
      </Button>
      <Button type="nav" icon="text" @click="openSettings" v-show="uiStore.showSettings !== true">
        <h3>菜单</h3>
      </Button>
    </div>
    <GameExtraUI />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, watch, nextTick } from 'vue'
import FreeModeTools from '@/components/tools/FreeModeTools.vue'
import { useUIStore } from '../../stores/modules/ui/ui'
import { useGameStore } from '../../stores/modules/game'
import { useUserStore } from '../../stores/modules/user/user'
import { GameBackground, GameRolesStage } from '../game/standard'
import { GameDialog } from '../game/standard'
import { Button } from '../base'

import GameExtraUI from '../game/standard/GameExtraUI.vue'

const uiStore = useUIStore()
const gameStore = useGameStore()
const userStore = useUserStore()

const gameDialogRef = ref<InstanceType<typeof GameDialog> | null>(null)

const openSettings = () => {
  uiStore.toggleSettings(true)
  uiStore.setSettingsTab('text')
}

const switchAutoMode = () => {
  uiStore.autoMode = !uiStore.autoMode
}

const runInitialization = async () => {
  const userId = '1' // TODO: 获取真实 userId

  try {
    await gameStore.initializeGame(userStore.client_id, userId)
  } catch (error) {
    console.log(error)
  }
}

// 初始化游戏信息
onMounted(() => {
  if (userStore.client_id !== '') {
    runInitialization()
  }
})

// 监听 client_id 的变化
watch(
  () => userStore.client_id,
  (newId) => {
    if (newId) {
      runInitialization()
    }
  },
)

/* 以下代码为自动AUTO模式逻辑 比较复杂 */
// 1. 用于存储 setTimeout 返回的 ID
let timerId: any = null
// 2. 状态标志，记录 continue() 是否已被调用
const isContinueTriggered = ref(false)
// 3. 追踪音频和打字状态
const audioFinished = ref(true) // 默认 true（无音频时视为已完成）

// 在新交互开始前调用的重置函数
const resetInteraction = () => {
  isContinueTriggered.value = false
  audioFinished.value = true
  if (timerId) {
    clearTimeout(timerId)
    timerId = null
  }
}

// 尝试触发自动继续（打字和音频都结束后才执行）
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

// 音频开始播放
const handleAudioStarted = () => {
  audioFinished.value = false
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
    console.log('父组件：打字状态变化', typing)
    if (typing === false) {
      tryAutoAdvance()
    }
  },
)

// 用户手动触发的函数
const manualTriggerContinue = () => {
  console.log('用户主动点击了')
  if (timerId) {
    clearTimeout(timerId)
    timerId = null
    console.log('父组件：已取消自动继续的定时器。')
  }

  if (!isContinueTriggered.value) {
    isContinueTriggered.value = true
  } else {
    console.log('父组件：用户重复点击，但方法已执行过，不再调用。')
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

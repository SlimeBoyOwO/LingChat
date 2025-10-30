<template>
  <div class="main-box">
    <GameBackground></GameBackground>
    <GameAvatar ref="gameAvatarRef" @audio-ended="handleAudioFinished" />
    <GameDialog
      ref="gameDialogRef"
      @player-continued="manualTriggerContinue"
      @dialog-proceed="resetInteraction"
    />
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
      <Button
        type="nav"
        icon="text"
        @click="openSettings"
        v-show="uiStore.showSettings !== true"
      >
        <h3>菜单</h3>
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useUIStore } from "../../stores/modules/ui/ui";
import { useGameStore } from "../../stores/modules/game";
import { GameBackground } from "../game/standard";
import { GameAvatar } from "../game/standard";
import { GameDialog } from "../game/standard";
import { getGameInfo } from "../../api/services/game-info";
import { Button } from "../base";

const uiStore = useUIStore();
const gameStore = useGameStore();

const gameAvatarRef = ref<InstanceType<typeof GameAvatar> | null>(null);
const gameDialogRef = ref<InstanceType<typeof GameDialog> | null>(null);

const openSettings = () => {
  uiStore.toggleSettings(true);
  uiStore.setSettingsTab("text");
};

const switchAutoMode = () => {
  uiStore.autoMode = !uiStore.autoMode;
};

const runInitialization = async () => {
  const userId = "1"; // TODO: 获取真实 userId
  try {
    await gameStore.initializeGame(userId);

    // Action 成功后，处理仅与本组件相关的 UI 逻辑
    if (gameAvatarRef.value) {
      gameAvatarRef.value.setEmotion("正常");
    } else {
      console.log("这个组件不存在");
    }
  } catch (error) {
    console.log(error);
  }
};

// 初始化游戏信息
onMounted(runInitialization);

// 当选中的角色ID改变时，重新执行初始化。
watch(
  () => gameStore.avatar.character_id,
  (newId, oldId) => {
    if (newId && newId !== oldId) {
      console.log(`Character ID changed to ${newId}, re-initializing...`);
      runInitialization();
    }
  }
);

/* 以下代码为自动AUTO模式逻辑 比较复杂 */
// 1. 用于存储 setTimeout 返回的 ID
let timerId: any = null;
// 2. 状态标志，记录 continue() 是否已被调用
const isContinueTriggered = ref(false);

// 在新交互开始前调用的重置函数
const resetInteraction = () => {
  isContinueTriggered.value = false;
  if (timerId) {
    clearTimeout(timerId);
    timerId = null;
  }
};

// 自动播放功能
const handleAudioFinished = () => {
  if (!uiStore.autoMode) return;
  if (isContinueTriggered.value) {
    console.log("父组件：音频结束了，但用户已手动继续，不做任何事。");
    return;
  }
  if (gameStore.currentStatus !== "responding") return;

  if (timerId) clearTimeout(timerId);

  timerId = setTimeout(() => {
    if (gameDialogRef.value) {
      const needWait = gameDialogRef.value.continueDialog(false);
      if (needWait) {
        handleAudioFinished();
      }
    } else {
      console.error("无法找到 GameDialog 的实例。");
    }
  }, 1000);
};

// 用户手动触发的函数
const manualTriggerContinue = () => {
  // 5. 立即清除定时器，阻止其后续执行
  console.log("用户主动点击了");
  if (timerId) {
    clearTimeout(timerId);
    timerId = null;
    console.log("父组件：已取消自动继续的定时器。");
  }

  // 6. 检查状态，防止重复执行
  if (!isContinueTriggered.value) {
    isContinueTriggered.value = true; // 设置标志
  } else {
    console.log("父组件：用户重复点击，但方法已执行过，不再调用。");
  }
};
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
</style>

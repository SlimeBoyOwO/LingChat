<template>
  <div class="main-box">
    <GameBackground></GameBackground>
    <GameAvatar ref="gameAvatarRef" />
    <GameDialog />
    <div id="menu-panel">
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

const openSettings = () => {
  uiStore.toggleSettings(true);
  uiStore.setSettingsTab("text");
};

const runInitialization = async () => {
  const userId = "1"; // TODO: 获取真实 userId
  try {
    await gameStore.initializeGame(userId);

    // Action 成功后，处理仅与本组件相关的 UI 逻辑
    if (gameAvatarRef.value) {
      gameAvatarRef.value.setEmotion("正常", true);
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
  position: fixed;
  top: 15px;
  right: 20px;
  z-index: 1000;
}
</style>

<template>
  <div class="main-box">
    <GameBackground></GameBackground>
    <GameAvatar ref="gameAvatarRef" />
    <GameDialog />
    <div id="menu-panel">
      <button
        id="menu-toggle"
        @click="openSettings"
        v-show="uiStore.showSettings !== true"
      >
        菜单
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useUIStore } from "../../stores/modules/ui/ui";
import { useGameStore } from "../../stores/modules/game";
import { GameBackground } from "../game/standard";
import { GameAvatar } from "../game/standard";
import { GameDialog } from "../game/standard";
import { getGameInfo } from "../../api/services/game-info";

const uiStore = useUIStore();
const gameStore = useGameStore();

const gameAvatarRef = ref<InstanceType<typeof GameAvatar> | null>(null);

const openSettings = () => {
  uiStore.toggleSettings(true);
  uiStore.setSettingsTab("text");
};

// 初始化游戏信息
onMounted(async () => {
  try {
    // TODO: 从用户store或其他地方获取userId
    const userId = "1"; // 临时使用固定值，实际应该从用户认证信息中获取
    const gameInfo = await getGameInfo(userId);

    // 更新 gameStore 中的角色信息
    gameStore.avatar.character_name = gameInfo.ai_name;
    gameStore.avatar.character_subtitle = gameInfo.ai_subtitle;
    gameStore.avatar.user_name = gameInfo.user_name;
    gameStore.avatar.user_subtitle = gameInfo.user_subtitle;
    gameStore.avatar.character_id = gameInfo.character_id;
    gameStore.avatar.think_message = gameInfo.thinking_message;

    // 初始化UI界面主角的信息
    uiStore.showCharacterTitle = gameInfo.user_name;
    uiStore.showCharacterSubtitle = gameInfo.user_subtitle;

    // 触发GameAvatar的setEmotion函数
    if (gameAvatarRef.value) {
      gameAvatarRef.value.setEmotion("正常", true);
    } else {
      console.log("这个组件不存在");
    }
  } catch (error) {
    console.error("初始化游戏信息失败:", error);
    // 可以在这里添加错误处理，比如显示错误提示
  }
});
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

#menu-toggle {
  padding: 12px 20px;
  background-color: #d2e5ff;
  color: #3d8dff;
  font-size: 20px;
  font-weight: bold;
  border: 0.5px solid #e5ecf1;
  /* 银灰色边框 */
  border-radius: 8px;
  /* 稍微增大圆角 */
  cursor: pointer;
  box-shadow: 0 4px 8px rgba(0, 0, 0, 0.15);
  /* 底部阴影 */
  transition: all 0.3s ease;
  /* 添加过渡效果 */
}

#menu-toggle:hover {
  background-color: #b8d7ff;
  box-shadow: 0 6px 12px rgba(0, 0, 0, 0.2);
  /* 悬停时阴影更明显 */
  transform: translateY(-2px);
  /* 轻微上浮效果 */
}

#menu-panel {
  position: fixed;
  top: 30px;
  right: 30px;
  z-index: 1000;
}
</style>

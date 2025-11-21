<template>
  <div id="app" @mouseenter="handleMouseEnter" @mouseleave="handleMouseLeave">
    <!-- 对话框组件 -->
    <DialogueBox />

    <!-- 头像容器组件 -->
    <AvatarContainer @avatar-click="handleAvatarClick" />

    <!-- 主按钮组件 -->
    <MainButton @click="handleMainButtonClick" />

    <!-- 对话输入框组件 -->
    <ChatInput :visible="showChatInput" @message-sent="handleMessageSent" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import { getCurrentWindow, Window, LogicalSize } from "@tauri-apps/api/window";
import { useGameStore } from "../../stores/modules/game";

// 导入组件
import AvatarContainer from "../game/AvatarContainer.vue";
import MainButton from "../game/MainButton.vue";
import ChatInput from "../game/ChatInput.vue";
import DialogueBox from "../game/DialogueBox.vue";
import { eventQueue } from "../../core/events/event-queue";

// 使用ref存储窗口实例
const mainWindow = ref<Window | null>(null);
const gameStore = useGameStore();

const runInitialization = async () => {
  // 初始化窗口实例
  mainWindow.value = await getCurrentWindow();

  const userId = "1"; // TODO: 获取真实 userId
  try {
    await gameStore.initializeGame(userId);

    // Action 成功后，处理仅与本组件相关的 UI 逻辑
    // if (gameAvatarRef.value) {
    //   gameAvatarRef.value.setEmotion("正常");
    // } else {
    //   console.log("这个组件不存在");
    // }
  } catch (error) {
    console.log(error);
  }
};

// 初始化游戏信息
onMounted(runInitialization);

// 对话输入框控制
const showChatInput = ref(false);

// 监听输入框显示状态变化，动态调整窗口大小
const adjustWindowSize = async (visible: boolean) => {
  try {
    if (!mainWindow.value) {
      console.warn("主窗口未初始化");
      return;
    }

    if (visible) {
      // 显示输入框时，扩展窗口高度40px
      await mainWindow.value.setSize(new LogicalSize(260, 380));
    } else {
      // 隐藏输入框时，恢复原始窗口大小
      await mainWindow.value.setSize(new LogicalSize(260, 380));
    }
  } catch (error) {
    console.error("调整窗口大小失败:", error);
  }
};

// 监听showChatInput变化
watch(showChatInput, (newValue) => {
  adjustWindowSize(newValue);
});

// 处理主按钮点击事件
const handleMainButtonClick = () => {
  // 主按钮点击处理逻辑已移至MainButton组件内部
  // 这里可以添加额外的处理逻辑
  console.log("主按钮被点击");
};

// 处理消息发送事件
const handleMessageSent = (message: string) => {
  // scriptHandler.sendMessage(message);
};

// 处理鼠标进入事件
const handleMouseEnter = () => {
  showChatInput.value = true;
};

// 处理鼠标离开事件
const handleMouseLeave = () => {
  showChatInput.value = false;
};

// 处理头像点击事件
const handleAvatarClick = () => {
  console.log("Main: 头像被点击，继续对话");
  eventQueue.continue();
};
</script>

<style scoped>
/* 主应用样式 */
#app {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
}
</style>

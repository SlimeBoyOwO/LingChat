<template>
  <div class="chat-input-container" :class="{ visible: props.visible }">
    <div class="chat-input-glass">
      <input
        v-model="messageText"
        type="text"
        :placeholder="placeholderText"
        :readonly="!isInputEnabled"
        class="chat-input"
        @keyup.enter="sendMessage"
      />
    </div>
    <button class="send-button" @click="sendMessage">
      <svg
        xmlns="http://www.w3.org/2000/svg"
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <line x1="22" y1="2" x2="11" y2="13"></line>
        <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
      </svg>
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from "vue";
import { scriptHandler } from "../../api/websocket/handlers/script-handler";
import { useGameStore } from "../../stores/modules/game";
import { useUIStore } from "../../stores/modules/ui/ui";

const gameStore = useGameStore();
const uiStore = useUIStore();

// 使用计算属性处理占位符文本
const placeholderText = computed(() => {
  switch (gameStore.currentStatus) {
    case "input":
      return uiStore.showPlayerHintLine || "输入消息...";
    case "thinking":
      return gameStore.avatar.think_message;
    case "responding":
      return "聊天ing~";
    case "presenting":
      return "";
    default:
      return "输入消息...";
  }
});

// 监听状态变化 TODO: 这里不应该在这个单元执行
watch(
  () => gameStore.currentStatus,
  (newStatus) => {
    console.log("游戏状态变为 :", newStatus);
    if (newStatus === "thinking") {
      gameStore.avatar.emotion = "AI思考";
    } else if (newStatus === "input") {
      uiStore.showCharacterEmotion = "";
    }
  }
);

// 使用计算属性控制输入框是否可编辑
const isInputEnabled = computed(() => gameStore.currentStatus === "input");

// 定义组件属性
const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
});

// 定义事件
const emit = defineEmits(["message-sent"]);

// 消息文本
const messageText = ref("");

// 发送消息函数
const sendMessage = () => {
  if (messageText.value.trim()) {
    scriptHandler.sendMessage(messageText.value);
    // 触发事件通知父组件
    emit("message-sent", messageText.value);
    messageText.value = "";
    gameStore.currentStatus = "thinking";
    gameStore.addToDialogHistory({
      type: "message",
      character: gameStore.avatar.user_name,
      content: messageText,
    });
  }
};
</script>

<style scoped>
/* 对话输入框样式 */
.chat-input-container {
  position: relative;
  top: 150%;
  left: 10%;
  width: 100%;
  opacity: 0;
  visibility: hidden;
  transition: all 0.3s ease;
  z-index: 10;
  display: flex;
}

.chat-input-container.visible {
  top: -12px;
  opacity: 1;
  visibility: visible;
}

.chat-input-glass {
  /* 液态玻璃效果 */
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px) saturate(180%);
  -webkit-backdrop-filter: blur(10px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 20px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
  padding: 0px 4px;
  display: flex;
  align-items: center;
}

.chat-input {
  background: transparent;
  border: none;
  outline: none;
  color: white;
  flex: 1;
  font-size: 13px;
  padding: 5px;
}

.chat-input::placeholder {
  color: rgba(255, 255, 255, 0.6);
}

.send-button {
  background: rgba(255, 255, 255, 0.2);
  border: none;
  border-radius: 50%;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: white;
  transition: all 0.2s ease;
  margin-left: 5px;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 20px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
}

.send-button:hover {
  background: rgba(255, 255, 255, 0.3);
  transform: scale(1.05);
}

.send-button:active {
  transform: scale(0.95);
}
</style>

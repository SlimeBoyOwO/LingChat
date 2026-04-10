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
      <button
        class="h-6 px-2 bg-gradient-to-tr from-cyan-500 to-blue-400 hover:from-cyan-400 hover:to-blue-300 text-white font-bold text-sm rounded-full shadow-[0_4px_15px_rgba(6,182,212,0.4)] hover:shadow-[0_6px_20px_rgba(6,182,212,0.6)] transition-all duration-300 active:scale-95 flex items-center gap-1 overflow-hidden relative"
        @click="sendMessage"
      >
        <div
          class="absolute top-0 left-0 w-full h-1/2 bg-white/20 rounded-t-full pointer-events-none"
        ></div>
        <svg
          xmlns="http://www.w3.org/2000/svg"
          class="h-4 w-4"
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path
            d="M10.894 2.553a1 1 0 00-1.788 0l-7 14a1 1 0 001.169 1.409l5-1.429A1 1 0 009 15.571V11a1 1 0 112 0v4.571a1 1 0 00.725.962l5 1.428a1 1 0 001.17-1.408l-7-14z"
          />
        </svg>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from "vue";
import { scriptHandler } from "../../api/websocket/handlers/script-handler";
import { useGameStore } from "../../stores/modules/game";
import { useUIStore } from "../../stores/modules/ui/ui";

const gameStore = useGameStore();
const uiStore = useUIStore();

const placeholderText = computed(() => {
  switch (gameStore.currentStatus) {
    case "input":
      return uiStore.showPlayerHintLine || "输入消息...";
    case "thinking":
      const currentInteractRole = gameStore.currentInteractRole;
      if (currentInteractRole) {
        return currentInteractRole.thinkMessage;
      } else {
        return "等待回应中...";
      }
    case "responding":
      return "聊天ing~";
    case "presenting":
      return "";
    default:
      return "在这里输入消息...";
  }
});

// 监听状态变化 TODO: 这里不应该在这个单元执行
watch(
  () => gameStore.currentStatus,
  (newStatus) => {
    console.log("游戏状态变为 :", newStatus);
    if (newStatus === "thinking") {
      const currentInteractRole = gameStore.currentInteractRole;
      if (currentInteractRole) {
        currentInteractRole.emotion = "AI思考";
        uiStore.showCharacterTitle = currentInteractRole.roleName;
        uiStore.showCharacterSubtitle = currentInteractRole.roleSubTitle;
      }
    } else if (newStatus === "input") {
      uiStore.showCharacterEmotion = "";
    }
  },
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
    gameStore.appendGameMessage({
      type: "message",
      displayName: gameStore.userName,
      content: messageText.value,
    });
  }
};
</script>

<style scoped>
/* 对话输入框样式 */
.chat-input-container {
  position: relative;
  width: 100%;
  opacity: 0;
  visibility: hidden;
  transform: scale(var(--pet-ui-scale, 1));
  transition:
    opacity 0.3s ease,
    visibility 0.3s ease;
  z-index: 10;
  display: flex;
}

.chat-input-container.visible {
  opacity: 1;
  visibility: visible;
}

.chat-input-glass {
  /* 液态玻璃效果 */
  background: rgba(255, 255, 255, 0.05);
  backdrop-filter: blur(10px) saturate(180%);
  -webkit-backdrop-filter: blur(10px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: calc(20px * var(--pet-ui-scale, 1));
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
  padding: 0 calc(4px * var(--pet-ui-scale, 1));
  display: flex;
  align-items: center;
}

.chat-input {
  background: transparent;
  border: none;
  outline: none;
  color: white;
  flex: 1;
  font-size: calc(13px * var(--pet-ui-scale, 1));
  padding: calc(5px * var(--pet-ui-scale, 1));
}

.chat-input::placeholder {
  color: rgba(255, 255, 255, 0.6);
}
</style>

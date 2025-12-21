<template>
  <div class="chatbox-box">
    <div class="chatbox-main">
      <div class="chatbox-title-part">
        <div class="chatbox-title">
          <div id="character">{{ uiStore.showCharacterTitle }}</div>
        </div>
        <div class="chatbox-subtitle">
          <div id="character-sub">{{ uiStore.showCharacterSubtitle }}</div>
        </div>
        <div class="chatbox-emotion">
          <div id="character-emotion">{{ uiStore.showCharacterEmotion }}</div>
        </div>
        <Button
          type="nav"
          icon="history"
          title=""
          @click="openHistory"
        ></Button>
      </div>
      <div class="chatbox-line"></div>
      <div class="chatbox-inputbox">
        <textarea
          id="inputMessage"
          ref="textareaRef"
          :placeholder="placeholderText"
          v-model="inputMessage"
          @keydown.enter.exact.prevent="sendOrContinue"
          :readonly="!isInputEnabled"
        ></textarea>
        <button id="sendButton" :disabled="isSending" @click="sendOrContinue">
          ▼
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from "vue";
import { Button } from "../../base";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { useTypeWriter } from "../../../composables/ui/useTypeWriter";
import { eventQueue } from "../../../core/events/event-queue";
import { scriptHandler } from "../../../api/websocket/handlers/script-handler";

const inputMessage = ref("");
const textareaRef = ref<HTMLTextAreaElement | null>(null); // 新增这行
const gameStore = useGameStore();
const uiStore = useUIStore();

const { startTyping, stopTyping, isTyping } = useTypeWriter(textareaRef);

// 使用计算属性处理发送状态
const isSending = computed(() => gameStore.currentStatus === "thinking");

const emit = defineEmits(["player-continued", "dialog-proceed"]);

const openHistory = () => {
  uiStore.toggleSettings(true);
  uiStore.setSettingsTab("history");
};

// 使用计算属性处理占位符文本
const placeholderText = computed(() => {
  switch (gameStore.currentStatus) {
    case "input":
      return uiStore.showPlayerHintLine || "在这里输入消息...";
    case "thinking":
      return gameStore.avatar.think_message;
    case "responding":
    case "presenting":
      return "";
    default:
      return "在这里输入消息...";
  }
});

// 使用计算属性控制输入框是否可编辑
const isInputEnabled = computed(() => gameStore.currentStatus === "input");

// 监听状态变化
watch(
  () => gameStore.currentStatus,
  (newStatus) => {
    console.log("游戏状态变为 :", newStatus);
    if (newStatus === "thinking") {
      gameStore.avatar.emotion = "AI思考";
      uiStore.showCharacterTitle = gameStore.avatar.character_name;
      uiStore.showCharacterSubtitle = gameStore.avatar.character_subtitle;
    } else if (newStatus === "input") {
      uiStore.showCharacterTitle = gameStore.avatar.user_name;
      uiStore.showCharacterSubtitle = gameStore.avatar.user_subtitle;
      uiStore.showCharacterEmotion = "";
    } else if (newStatus === "presenting") {
      uiStore.showCharacterTitle = "";
      uiStore.showCharacterSubtitle = "";
      uiStore.showCharacterEmotion = "";
      uiStore.showCharacterLine = "";
    }
  }
);

// 监听 currentLine 和 currentStatus 的变化
watch(
  [() => uiStore.showCharacterLine, () => gameStore.currentStatus],
  ([newLine, newStatus]) => {
    if (newLine && newLine !== "" && newStatus === "responding") {
      inputMessage.value = "";
      startTyping(newLine, uiStore.typeWriterSpeed);
    } else if (newStatus === "input") {
      // 只要进入 input 状态就清空，不管 newLine 是什么
      stopTyping();
      inputMessage.value = "";
    }
  }
);

function sendOrContinue() {
  if (gameStore.currentStatus === "input") {
    send();
  } else if (gameStore.currentStatus === "responding") {
    continueDialog(true);
  }
}

function send() {
  const text = inputMessage.value;
  if (!text.trim()) return;
  if (text === "/开始剧本") {
    gameStore.initializeScript("TODO: 从剧本面板选择剧本");
    // TODO: 清空背景，清空人物
    gameStore.avatar.show = false;
    gameStore.script.isRunning = true;
  } else {
    gameStore.currentStatus = "thinking";
    gameStore.addToDialogHistory({
      type: "message",
      character: gameStore.avatar.user_name,
      content: text,
    });
  }

  scriptHandler.sendMessage(text);
  inputMessage.value = "";
}

function continueDialog(isPlayerTrigger: boolean): boolean {
  const needWait = eventQueue.continue();
  if (!needWait) {
    if (isPlayerTrigger) emit("player-continued");
    emit("dialog-proceed");
  }

  return needWait;
}

defineExpose({
  continueDialog,
});
</script>

<style>
.chatbox-box {
  position: relative;
  display: flex;
  justify-content: center;
  width: 100%;
  z-index: 2;
  background: linear-gradient(
    to top,
    rgba(0, 14, 39, 0.7) 0%,
    rgba(0, 14, 39, 0.6) 100%
  );
  padding: 15px;
  backdrop-filter: blur(1px);
  scrollbar-width: thin;
  scrollbar-color: var(--accent-color) transparent;
}

.chatbox-box::before {
  content: "";
  position: absolute;
  top: -40px;
  left: 0;
  right: 0;
  height: 40px;
  background: linear-gradient(
    to bottom,
    transparent 0%,
    rgba(0, 14, 39, 0.3) 50%,
    rgba(0, 14, 39, 0.6) 100%
  );
  pointer-events: none;
}

.chatbox-main {
  width: 60%;
}

.chatbox-title-part {
  display: flex;
  align-items: baseline;
  margin-bottom: 10px;
}

/* 确保所有文本元素都继承相同的字体和文字阴影 */
.chatbox-title,
.chatbox-subtitle,
#inputMessage,
#sendButton {
  font-family: inherit; /* 继承父元素字体 */
  text-shadow: inherit; /* 继承文字阴影 */
}

/* 调整特定元素的字体大小和粗细 */
.chatbox-title {
  font-size: 24px;
  font-weight: bold;
  color: white;
  margin-right: 15px;
}

.chatbox-subtitle {
  font-size: 20px;
  font-weight: bold;
  color: #6eb4ff;
}

.chatbox-emotion {
  font-size: 20px;
  font-weight: bold;
  color: #ff77dd;
  margin: auto;
}

.chatbox-line {
  height: 1px;
  background: rgba(255, 255, 255, 0.3);
  margin: 6px 0 6px 0;
}

.chatbox-inputbox {
  display: flex;
  flex-direction: column;
  white-space: pre-line;
  width: 100%;
  min-height: 40px;
  background: rgba(255, 255, 255, 0);
  border: none;
  color: white;
  font-size: 20px;
  font-weight: bold;
  resize: none;
  margin: 5px 0px;
  outline: none;
  transition: all 0.3s;
}

#inputMessage {
  font-size: 20px;
  font-weight: bold;
  width: 100%;
  min-height: 40px;
  background: rgba(255, 255, 255, 0);
  border: none;
  color: white;
  font-size: 20px;
  font-weight: bold;
  resize: none;
  margin: 5px 0px;
  outline: none;
  transition: all 0.3s;
}

#inputMessage::placeholder {
  color: rgba(255, 255, 255, 0.5); /* 明亮的灰色 */
  text-shadow: none; /* 移除阴影 */
}

#sendButton {
  align-self: flex-end;
  background: rgba(0, 14, 39, 0);
  color: rgb(4, 188, 255);
  border: none;
  padding: 4px 10px;
  border-radius: 5px;
  cursor: pointer;
  transition: all 0.3s;
  font-size: 20px;
  font-weight: bold;
  transform: scaleX(1.5);
}

#sendButton:hover {
  background: rgba(0, 14, 39, 0);
  color: rgba(136, 255, 251, 0.827);
}

#sendButton:disabled {
  background: #333;
  cursor: not-allowed;
  opacity: 0.7;
}
</style>

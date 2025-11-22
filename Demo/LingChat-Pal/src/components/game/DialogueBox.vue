<template>
  <div class="dialogue-box" ref="dialogueBox">
    <div class="dialogue-content">
      <div class="character-emotion">{{ characterEmotion }}</div>
      <div class="dialogue-text">{{ dialogueText }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useGameStore } from "../../stores/modules/game";
import { eventQueue } from "../../core/events/event-queue";

// 获取游戏状态
const gameStore = useGameStore();
const dialogueBox = ref<HTMLElement | null>(null);

// 计算属性：判断对话框是否可见
const isVisible = computed(() => {
  return (
    gameStore.currentStatus === "responding" &&
    gameStore.currentLine.trim() !== ""
  );
});

// 计算属性：获取角色名称
const characterName = computed(() => {
  return gameStore.character || gameStore.avatar.character_name;
});

// 计算属性：获取角色情绪
const characterEmotion = computed(() => {
  return gameStore.avatar.emotion || "正常";
});

// 计算属性：获取对话文本
const dialogueText = computed(() => {
  return gameStore.currentLine || "";
});

// 监听对话框可见性变化
watch(isVisible, (newValue) => {
  if (dialogueBox.value) {
    // 修改opcaity
    dialogueBox.value.style.opacity = newValue ? "1" : "0";
  }
  console.log("对话框可见性变化:", newValue);
});

// 处理对话框点击事件
const handleDialogueClick = () => {
  if (isVisible.value) {
    console.log("点击对话框，继续下一句");
    eventQueue.continue();
  }
};
</script>

<style scoped>
.dialogue-box {
  position: relative;
  top: 10px; /* 距离窗口顶部10px */
  left: 50%;
  transform: translateX(-50%);
  width: 100%; /* 限制宽度不超过窗口 */
  height: 70px;
  z-index: 30; /* 确保在最顶层 */
  cursor: pointer;
  animation: fadeIn 0.3s ease-in-out;
}

.dialogue-content {
  /* 液态玻璃效果 */
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px) saturate(180%);
  -webkit-backdrop-filter: blur(10px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 20px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
  padding: 12px;
  color: white;
  transition: all 0.2s ease;
}

.dialogue-content:hover {
  background: rgba(255, 255, 255, 0.15);
  transform: translateY(-2px);
}

.character-name {
  font-weight: bold;
  font-size: 16px;
}

.character-emotion {
  font-size: 12px;
  opacity: 0.8;
  font-style: italic;
  right: 0;
}

.dialogue-text {
  font-size: 15px;
  line-height: 1.5;
  white-space: pre-wrap;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--accent-color) transparent;
  -ms-overflow-style: -ms-autohiding-scrollbar;
  min-height: 25px;
  max-height: 45px;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateX(-50%) translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
  }
}
</style>

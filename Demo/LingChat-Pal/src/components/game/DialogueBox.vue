<template>
  <div class="dialogue-box" @click="handleDialogueClick" ref="dialogueBox">
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
import { useUIStore } from "../../stores/modules/ui/ui";

// 获取游戏状态
const gameStore = useGameStore();
const uiStore = useUIStore();
const dialogueBox = ref<HTMLElement | null>(null);

// 计算属性：判断对话框是否可见
const isVisible = computed(() => {
  return (
    gameStore.currentStatus === "responding" &&
    gameStore.currentLine.trim() !== ""
  );
});

// 计算属性：获取角色名称
// const characterName = computed(() => {
//   return uiStore.showCharacterTitle;
// });

// 计算属性：获取角色情绪
const characterEmotion = computed(() => {
  return uiStore.showCharacterEmotion ? uiStore.showCharacterEmotion : "";
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
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 30;
  cursor: pointer;
  animation: fadeIn 0.3s ease-in-out;
  opacity: 0;
}

.dialogue-content {
  /* 高级液态玻璃与渐变辉光效果 */
  background: linear-gradient(135deg, rgba(255, 255, 255, 0.15) 0%, rgba(255, 255, 255, 0.05) 100%);
  backdrop-filter: blur(12px) saturate(200%);
  -webkit-backdrop-filter: blur(12px) saturate(200%);
  border: 1px solid rgba(255, 255, 255, 0.3);
  border-radius: calc(20px * var(--pet-ui-scale, 1));
  box-shadow:
    0 10px 40px -10px rgba(0, 176, 255, 0.3),
    inset 0 1px 2px rgba(255, 255, 255, 0.4);
  padding: calc(14px * var(--pet-ui-scale, 1)) calc(18px * var(--pet-ui-scale, 1));
  width: 85%;
  color: white;
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  position: relative;
  overflow: hidden;
}

/* 内部高亮扫光效果 */
.dialogue-content::after {
  content: "";
  position: absolute;
  top: 0;
  left: -100%;
  width: 50%;
  height: 100%;
  background: linear-gradient(90deg, transparent, rgba(255,255,255,0.2), transparent);
  animation: shine 4s ease-in-out infinite;
  pointer-events: none;
}

@keyframes shine {
  0% { left: -100%; }
  20% { left: 100%; }
  100% { left: 100%; }
}

.dialogue-content:hover {
  background: linear-gradient(135deg, rgba(255, 255, 255, 0.2) 0%, rgba(255, 255, 255, 0.1) 100%);
  transform: scale(1.02) translateY(-2px);
  box-shadow:
    0 15px 50px -10px rgba(0, 176, 255, 0.5),
    inset 0 1px 2px rgba(255, 255, 255, 0.5);
  border-color: rgba(255, 255, 255, 0.5);
}

.character-name {
  font-weight: bold;
  font-size: 16px;
}

.character-emotion {
  font-size: calc(12px * var(--pet-ui-scale, 1));
  color: rgba(165, 243, 252, 0.9);
  font-weight: 600;
  font-style: italic;
  letter-spacing: 1px;
  margin-bottom: 2px;
  text-shadow: 0 1px 4px rgba(0, 176, 255, 0.5);
}

.dialogue-text {
  font-size: calc(15px * var(--pet-ui-scale, 1));
  line-height: 1.5; /* Slightly tighter leading */
  font-weight: 500;
  white-space: pre-wrap;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--accent-color) transparent;
  -ms-overflow-style: -ms-autohiding-scrollbar;
  min-height: calc(25px * var(--pet-ui-scale, 1));
  max-height: calc(45px * var(--pet-ui-scale, 1));
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>

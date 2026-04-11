<template>
  <!-- 外层容器：使用动态 class 控制显示隐藏和淡入淡出动画 -->
  <div @click="handleDialogueClick"
    class="relative flex items-center justify-center w-full h-full z-30 cursor-pointer transition-all duration-300 ease-out"
    :class="isVisible
      ? 'opacity-100 translate-y-0'
      : 'opacity-0 translate-y-2 pointer-events-none'
      ">
    <!-- 气泡主体内容 (玻璃拟态效果) -->
    <div
      class="relative w-[85%] rounded-[calc(20px*var(--pet-ui-scale,1))] px-[calc(18px*var(--pet-ui-scale,1))] py-[calc(14px*var(--pet-ui-scale,1))] text-white backdrop-blur-md backdrop-saturate-200 border bg-white/20 border-white/30 transition-all duration-300 hover:bg-linear-to-br hover:scale-[1.02] hover:-translate-y-0.5 hover:border-white/50">
      <!-- 气泡尾巴 (指向桌宠的三角形)，默认居中偏下 -->
      <div
        class="absolute -bottom-2.5 left-1/2 -translate-x-1/2 w-0 h-0 border-l-10 border-l-transparent border-r-10 border-r-transparent border-t-white/30 drop-shadow-md">
      </div>
      <!-- 气泡尾巴的内部叠加，保持玻璃通透感 -->
      <div
        class="absolute -bottom-2 left-1/2 -translate-x-1/2 w-0 h-0 border-l-8 border-l-transparent border-r-8 border-r-transparent border-t-8 border-t-white/10">
      </div>

      <!-- 角色情绪 -->
      <div v-if="characterEmotion"
        class="text-[calc(12px*var(--pet-ui-scale,1))] text-cyan-400 font-semibold italic tracking-wider mb-0.5 drop-shadow-[0_1px_4px_rgba(0,176,255,0.5)] truncate">
        {{ characterEmotion }}
      </div>

      <!-- 对话文本：强制单行不换行，超长显示省略号 -->
      <div ref="textareaRef" class="text-[calc(15px*var(--pet-ui-scale,1))] leading-snug font-medium"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useGameStore } from "../../stores/modules/game";
import { eventQueue } from "../../core/events/event-queue";
import { useUIStore } from "../../stores/modules/ui/ui";
import { useTypeWriter } from "../../composables/useTypeWriter";

// 获取游戏状态
const gameStore = useGameStore();
const uiStore = useUIStore();

const currentDisplayedText = ref("");

const emit = defineEmits(["player-continued", "dialog-proceed"]);

// 计算属性：判断对话框是否可见
const isVisible = computed(() => {
  return (
    gameStore.currentStatus === "responding" &&
    gameStore.currentLine.trim() !== ""
  );
});

const isTyping = computed(
  () =>
    uiStore.showCharacterLine !== "" &&
    currentDisplayedText.value !== uiStore.showCharacterLine,
);

// 计算属性：获取角色情绪
const characterEmotion = computed(() => {
  return uiStore.showCharacterEmotion ? uiStore.showCharacterEmotion : "";
});

// 处理对话框点击事件
const handleDialogueClick = () => {
  if (isVisible.value) {
    console.log("点击对话框，继续下一句");
    continueDialog(true);
    eventQueue.continue();
  }
};

const textareaRef = ref<HTMLTextAreaElement | null>(null);

const { startTyping, stopTyping } = useTypeWriter(textareaRef, (text) => {
  currentDisplayedText.value = text;
});

watch(
  [() => uiStore.showCharacterLine, () => gameStore.currentStatus],
  ([newLine, newStatus]) => {
    if (newLine && newLine !== "" && newStatus === "responding") {
      currentDisplayedText.value = "";
      startTyping(newLine, uiStore.typeWriterSpeed);
    } else if (newStatus === "input") {
      stopTyping();
      currentDisplayedText.value = "";
    }
  },
);

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
  isTyping,
});
</script>

<style scoped>
/* 所有样式都已成功迁移至 Tailwind CSS 类中，不再需要额外的 css */
</style>

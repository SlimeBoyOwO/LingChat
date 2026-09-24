<template>
  <!-- 整个 UI 层的根容器 -->
  <div class="pointer-events-none fixed top-0 left-0 z-999 h-full w-full">
    <!-- 1. 章节名称显示区域 -->
    <ChapterNameDisplay />

    <!-- 2. Galgame 选项显示区域 -->
    <GameChoices />

    <!-- 3. 剧本完成提示动画 -->
    <ScriptCompleteDisplay />

    <!-- 4. Script 图片显示区域 -->
    <ScriptPicDisplay />

    <!-- 5. 自由对话动画与提示区域 -->
    <ScriptFreeDialogueDisplay />

    <!-- 6. 声效控制面板 -->
    <SoundEffectPanel />

    <!-- 7. 好感度/负面情绪变化飘出标签 -->
    <AffectionChangeFloat />

    <!-- 8. 音频频谱可视化（右下角，与左下角声效按钮对称） -->
    <SpectrumVisualizer />
  </div>
</template>

<script setup lang="ts">
import ChapterNameDisplay from "./extra/ChapterNameDisplay.vue";
import GameChoices from "./extra/GameChoices.vue";
import ScriptCompleteDisplay from "./extra/ScriptCompleteDisplay.vue";
import ScriptPicDisplay from "./extra/ScriptPicDisplay.vue";
import ScriptFreeDialogueDisplay from "./extra/ScriptFreeDialogueDisplay.vue";
import SoundEffectPanel from "./extra/SoundEffectPanel.vue";
import AffectionChangeFloat from "./extra/AffectionChangeFloat.vue";
import SpectrumVisualizer from "./extra/SpectrumVisualizer.vue";
</script>

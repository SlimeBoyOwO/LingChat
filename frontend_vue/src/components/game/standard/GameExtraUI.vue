<template>
  <!-- 整个 UI 层的根容器 -->
  <div class="fixed top-0 left-0 w-full h-full pointer-events-none z-999">
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
  </div>
</template>

<script setup lang="ts">
import ChapterNameDisplay from './extra/ChapterNameDisplay.vue'
import GameChoices from './extra/GameChoices.vue'
import ScriptCompleteDisplay from './extra/ScriptCompleteDisplay.vue'
import ScriptPicDisplay from './extra/ScriptPicDisplay.vue'
import ScriptFreeDialogueDisplay from './extra/ScriptFreeDialogueDisplay.vue'
</script>

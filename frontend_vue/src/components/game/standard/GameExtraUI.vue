<template>
  <!-- 整个 UI 层的根容器 -->
  <div class="fixed top-0 left-0 w-full h-full pointer-events-none z-[999]">
    <!-- 1. 章节名称显示区域 -->
    <ChapterNameDisplay />

    <!-- 2. Galgame 选项显示区域 -->
    <GameChoices />

    <!-- 3. 剧本完成提示动画 -->
    <ScriptCompleteDisplay />
  </div>
</template>

<script setup lang="ts">
import ChapterNameDisplay from './extra/ChapterNameDisplay.vue'
import GameChoices from './extra/GameChoices.vue'
import ScriptCompleteDisplay from './extra/ScriptCompleteDisplay.vue'
</script>

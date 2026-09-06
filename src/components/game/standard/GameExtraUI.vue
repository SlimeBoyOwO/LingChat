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

    <!-- 5.5 强制选择（DDLC 式鼠标拖拽演出） -->
    <ForceChoice />

    <!-- 5.55 选词写诗：全屏接管输入并自行播放带 loop 点的 BGM -->
    <PoemGame />

    <!-- 5.6 恐怖特效层（压过角色立绘与对话框） -->
    <HorrorEffectsLayer />

    <!-- 6. 声效控制面板 -->
    <SoundEffectPanel />
  </div>
</template>

<script setup lang="ts">
  import ChapterNameDisplay from "./extra/ChapterNameDisplay.vue";
  import GameChoices from "./extra/GameChoices.vue";
  import ScriptCompleteDisplay from "./extra/ScriptCompleteDisplay.vue";
  import ScriptPicDisplay from "./extra/ScriptPicDisplay.vue";
  import ScriptFreeDialogueDisplay from "./extra/ScriptFreeDialogueDisplay.vue";
  import ForceChoice from "./extra/ForceChoice.vue";
  import PoemGame from "./extra/PoemGame.vue";
  import HorrorEffectsLayer from "./HorrorEffectsLayer.vue";
  import SoundEffectPanel from "./extra/SoundEffectPanel.vue";
</script>

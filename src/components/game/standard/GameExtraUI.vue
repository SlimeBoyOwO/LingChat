<template>
  <div class="fixed top-0 left-0 w-full h-full pointer-events-none z-999">
    <ChapterNameDisplay />
    <GameChoices />
    <ScriptCompleteDisplay />
    <ScriptPicDisplay />
    <ScriptFreeDialogueDisplay />
    <!-- 声效控制面板 -->
    <SoundEffectPanel />
  </div>
</template>

<script setup lang="ts">
import ChapterNameDisplay from './extra/ChapterNameDisplay.vue'
import GameChoices from './extra/GameChoices.vue'
import ScriptCompleteDisplay from './extra/ScriptCompleteDisplay.vue'
import ScriptPicDisplay from './extra/ScriptPicDisplay.vue'
import ScriptFreeDialogueDisplay from './extra/ScriptFreeDialogueDisplay.vue'
import SoundEffectPanel from './extra/SoundEffectPanel.vue'
</script>

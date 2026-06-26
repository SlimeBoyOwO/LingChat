<template>
  <!-- 仅自由对话模式显示：番茄钟 + 日程/高级功能 -->
  <div v-if="shouldShow" class="fixed top-5 left-5 z-2000">
    <!-- 番茄钟：绝对定位，不影响其他元素 -->
    <div class="absolute top-0 left-0">
      <PomodoroPanel />
    </div>
    <!-- 按钮组：在番茄钟右侧，不受展开影响 -->
    <div class="ml-24">
      <ScheduleAndAdvancedPanel />
    </div>
  </div>

  <!-- RoleMoodBadge 内部自己管理定位，无需额外容器 -->
  <RoleMoodBadge />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import PomodoroPanel from '@/components/pomodoro/PomodoroPanel.vue'
import ScheduleAndAdvancedPanel from '@/components/schedule/ScheduleAndAdvancedPanel.vue'
import RoleMoodBadge from './RoleMoodBadge.vue'

const gameStore = useGameStore()

const shouldShow = computed(() => {
  // 剧情模式不显示番茄钟/日程
  return !(gameStore.runningScript && gameStore.runningScript.isRunning)
})
</script>

<template>
  <!-- 仅自由对话模式显示：番茄钟 + 日程（并列、挨在一起） -->
  <div
    v-if="shouldShow"
    class="fixed top-5 left-5 z-2000 flex items-start gap-3 transition-all ease-in-out duration-300"
  >
    <PomodoroPanel />
    <SchedulePanel />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import PomodoroPanel from '@/components/pomodoro/PomodoroPanel.vue'
import SchedulePanel from '@/components/schedule/SchedulePanel.vue'

const gameStore = useGameStore()

const shouldShow = computed(() => {
  // 剧情模式不显示番茄钟/日程
  return !(gameStore.runningScript && gameStore.runningScript.isRunning)
})
</script>

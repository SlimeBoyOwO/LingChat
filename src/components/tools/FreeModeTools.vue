<template>
  <!-- 仅自由对话模式显示：番茄钟 + 日程 + 好感度（并列、挨在一起） -->
  <div
    v-if="shouldShow"
    class="fixed top-[calc(15px+var(--safe-area-inset-top))] left-5 z-2000 flex items-start gap-3 transition-all duration-300 ease-in-out"
  >
    <PomodoroPanel />
    <SchedulePanel />
    <AffectionPanel v-if="affectionEnabled" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useGameStore } from "@/stores/modules/game";
import { getEnvConfigByKey } from "@/api/services/config";
import PomodoroPanel from "@/components/pomodoro/PomodoroPanel.vue";
import SchedulePanel from "@/components/schedule/SchedulePanel.vue";
import AffectionPanel from "@/components/tools/AffectionPanel.vue";

const gameStore = useGameStore();

const shouldShow = computed(() => {
  // 剧情模式不显示番茄钟/日程/好感度
  return !(gameStore.runningScript && gameStore.runningScript.isRunning);
});

// 好感度系统总开关（高级设置→其他高级设置→好感度）：关闭后隐藏面板
const affectionEnabled = ref(true);
onMounted(async () => {
  try {
    const item = await getEnvConfigByKey("affection.enabled");
    affectionEnabled.value = item.value !== "false";
  } catch {
    affectionEnabled.value = true;
  }
});
</script>

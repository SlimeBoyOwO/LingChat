<template>
  <!-- 仅自由对话模式显示：地点 + 日程 + 好感度（并列、挨在一起） -->
  <div
    v-if="shouldShow"
    class="fixed top-[calc(15px+var(--safe-area-inset-top))] left-5 z-2000 flex items-start gap-3 transition-all duration-300 ease-in-out"
  >
    <!-- 地点：显示当前场景名，点开跳到设置里的场景管理 -->
    <Button
      type="nav"
      class="flex items-center gap-2 px-4 py-2 text-white transition-colors"
      v-show="!uiStore.showSettings"
      @click="openSceneSettings"
    >
      <Signpost :size="18" class="shrink-0" />
      <h3 class="m-0 hidden text-lg font-bold xl:block">{{ sceneName }}</h3>
    </Button>

    <!-- 番茄钟暂时从顶栏撤下：组件与后台计时逻辑原样保留在 components/pomodoro/，
         计时状态持久化在 localStorage，重新挂上即按原进度续跑 -->
    <SchedulePanel />
    <AffectionPanel v-if="affectionEnabled" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { Signpost } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { getEnvConfigByKey } from "@/api/services/config";
import Button from "@/components/base/widget/Button.vue";
import SchedulePanel from "@/components/schedule/SchedulePanel.vue";
import AffectionPanel from "@/components/tools/AffectionPanel.vue";

const gameStore = useGameStore();
const uiStore = useUIStore();
const { t } = useI18n();

const shouldShow = computed(() => {
  // 剧情模式不显示地点/日程/好感度
  return !(gameStore.runningScript && gameStore.runningScript.isRunning);
});

// 未选场景时显示「无」，按钮仍作为进入场景管理的入口
const sceneName = computed(
  () => gameStore.currentScene?.scene_name || t("settings.background.scene.none"),
);

const openSceneSettings = () => {
  uiStore.toggleSettings(true);
  uiStore.setSettingsTab("background");
};

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

// 开关保存后后端热更新上帝 Agent 配置并广播最新状态，面板即时显隐
let unlisten: (() => void) | null = null;
onMounted(async () => {
  unlisten = await listen<boolean>("affection:enabled-changed", (event) => {
    affectionEnabled.value = event.payload !== false;
  });
});
onUnmounted(() => {
  unlisten?.();
});
</script>

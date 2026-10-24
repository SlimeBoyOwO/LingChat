<template>
  <StartList>
    <StartLine v-for="(script, index) in currentPageScripts">
      <StartItem class="menu-subitem" :key="script.script_name" @click="selectScript(script)">
        <span class="inline-flex items-center gap-2">
          <span>{{ script.script_name }}</span>
          <PluginTag v-if="script.source && script.source !== 'game'" :source="script.source" />
        </span>
      </StartItem>
    </StartLine>

    <StartLine>
      <StartItem
        class="menu-subitem"
        v-for="n in pageSize - currentPageScripts.length"
        :key="'placeholder-' + n"
        disabled="true"
      >
        {{ "\u00A0" }}
      </StartItem>
    </StartLine>
    <!-- 分页控制 -->
    <StartLine>
      <StartItem class="menu-subitem" @click="currentPage--" :disabled="currentPage === 1">
        <
      </StartItem>
      <StartItem class="menu-subitem" disabled="true" style="font-size: 28px">
        {{ currentPage }} / {{ totalPages }}
      </StartItem>
      <StartItem class="menu-subitem" @click="currentPage++" :disabled="currentPage === totalPages">
        >
      </StartItem>
      <!-- 返回按钮 -->
      <StartItem @click="emit('back')">{{ $t("views.menu.back") }}</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { StartItem, StartLine, StartList } from "../base";
import PluginTag from "@/components/ui/PluginTag.vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { type ScriptSummary, startScript } from "@/api/services/script-info";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";

const emit = defineEmits<{
  (e: "back"): void;
}>();

const props = defineProps({
  scripts: {
    type: Array as () => ScriptSummary[],
    default: [],
  },
});

const router = useRouter();
const { t } = useI18n();
const gameStore = useGameStore();
const uiStore = useUIStore();

const currentPage = ref(1);
const pageSize = 3;

const selectScript = async (script: ScriptSummary) => {
  // 剧本互斥：已有剧本在跑时不再起跑新的（后端 start_script 有同款校验兜底）
  if (gameStore.runningScript) {
    uiStore.showNotification({
      type: "warning",
      title: t("settings.adventure.lock.title"),
      message: t("settings.adventure.lock.running"),
      skipTipsCheck: true,
    });
    return;
  }

  await router.push("/chat");
  gameStore.enterStoryMode(script.script_name);

  try {
    await startScript(script.script_name);
  } catch (error) {
    gameStore.exitStoryMode();
    console.error("启动剧本失败:", error);
    uiStore.showNotification({
      type: "error",
      title: t("settings.adventure.lock.title"),
      message: typeof error === "string" ? error : t("settings.adventure.lock.running"),
      skipTipsCheck: true,
    });
  }
};

const totalPages = computed(() => {
  return Math.ceil(props.scripts.length / pageSize);
});

const currentPageScripts = computed(() => {
  const start = (currentPage.value - 1) * pageSize;
  const end = start + pageSize;
  return props.scripts.slice(start, end);
});
</script>

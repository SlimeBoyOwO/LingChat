<template>
  <StartList>
    <StartLine v-for="(script, index) in currentPageScripts">
      <StartItem class="menu-subitem" :key="script.script_name" @click="selectScript(script)">
        <span class="inline-flex items-center gap-2">
          <span>{{ script.script_name }}</span>
          <PluginTag v-if="script.source && script.source !== 'game'" :source="script.source" />
        </span>
      </StartItem>
      <ScriptActions
        :script="script"
        :busy="resettingName !== null"
        :done="resetDoneName === script.script_name"
        @reset="resetMemory(script)"
      />
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
import type { ScriptSummary } from "@/api/services/script-info";
import ScriptActions from "@lingchat/dlc-ui/host/ScriptActions.vue";
import { useStoryEntry } from "@lingchat/dlc-ui/composables/useStoryEntry";
import { useI18n } from "vue-i18n";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";

const emit = defineEmits<{
  (e: "back"): void;
  (e: "script-state-reset"): void;
}>();

const props = defineProps({
  scripts: {
    type: Array as () => ScriptSummary[],
    default: [],
  },
});

const { t } = useI18n();
const gameStore = useGameStore();
const uiStore = useUIStore();

const currentPage = ref(1);
const pageSize = 3;
// 记忆重置和内容警告沿用 DLC SDK；入口互斥由宿主统一检查。
const {
  resettingName,
  resetDoneName,
  resetMemory,
  selectScript: selectDlcScript,
} = useStoryEntry(emit);

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

  await selectDlcScript(script);
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

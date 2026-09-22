<template>
  <StartList>
    <StartLine v-for="(script, index) in currentPageScripts">
      <StartItem
        class="menu-subitem"
        :key="script.script_name"
        :disabled="isPossessed"
        :title="isPossessed ? $t('ui.characterCard.scriptStartDisabledPossessed') : ''"
        @click="selectScript(script)"
      >
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
const gameStore = useGameStore();
const uiStore = useUIStore();
const { t } = useI18n();

const currentPage = ref(1);
const pageSize = 3;

/** 附身中禁止开始剧本：以扮演身份进剧本会让身份错乱，与后端校验同源 */
const isPossessed = computed(() => gameStore.possessedRoleId !== 0);

const selectScript = async (script: ScriptSummary) => {
  if (isPossessed.value) return;
  try {
    // 先等后端受理：附身/并发拒绝会在引擎起跑前同步返回 Err，此时绝不可进入剧本模式
    await startScript(script.script_name);
  } catch (error) {
    uiStore.showNotification({
      type: "warning",
      title: t("ui.characterCard.startFailedTitle"),
      message: String(error),
      skipTipsCheck: true,
    });
    return;
  }

  await router.push("/chat");
  gameStore.enterStoryMode(script.script_name);
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

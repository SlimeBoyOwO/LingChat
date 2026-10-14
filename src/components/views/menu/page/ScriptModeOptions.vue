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

const currentPage = ref(1);
const pageSize = 3;
// 记忆重置按钮状态：resettingName 防连点，resetDoneName 做 ✓ 短暂反馈
const { resettingName, resetDoneName, resetMemory, selectScript } = useStoryEntry(emit);

const totalPages = computed(() => {
  return Math.ceil(props.scripts.length / pageSize);
});

const currentPageScripts = computed(() => {
  const start = (currentPage.value - 1) * pageSize;
  const end = start + pageSize;
  return props.scripts.slice(start, end);
});
</script>

<template>
  <StartList ref="listRef">
    <StartLine v-for="script in currentPageScripts" :key="script.script_name">
      <StartItem class="menu-subitem" @click="selectScript(script)">
        <span class="inline-flex items-center gap-2">
          <span>{{ script.script_name }}</span>
          <PluginTag v-if="script.source && script.source !== 'game'" :source="script.source" />
        </span>
      </StartItem>
    </StartLine>

    <!-- 分页控制 -->
    <StartLine class="pagination-row flex flex-wrap items-center justify-center gap-4">
      <StartItem class="menu-subitem" @click="prevPage" :disabled="currentPage <= 1">
        &lt;
      </StartItem>
      <StartItem class="menu-subitem" disabled="true" style="font-size: 28px">
        {{ safePage }} / {{ totalPages }}
      </StartItem>
      <StartItem class="menu-subitem" @click="nextPage" :disabled="currentPage >= totalPages">
        &gt;
      </StartItem>
      <!-- 返回按钮 -->
      <StartItem class="menu-subitem" @click="emit('back')">{{ $t("views.menu.back") }}</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
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
const pageSize = ref(3);
const listRef = ref<InstanceType<typeof StartList> | null>(null);

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
  return Math.ceil(props.scripts.length / pageSize.value);
});

/* 状态兜底：窗口变化或异常点击后页码可能越界，渲染时再钳一次 */
const safePage = computed(() =>
  Math.min(Math.max(1, currentPage.value), Math.max(1, totalPages.value)),
);

const currentPageScripts = computed(() => {
  const start = (safePage.value - 1) * pageSize.value;
  const end = start + pageSize.value;
  return props.scripts.slice(start, end);
});

/* 按可用高度决定每页展示的剧本数量：用真实行高和 StartPage 内容区高度推算，
   分页行按实际高度扣除（窄屏折行时会自动变大） */
let resizeTimer = 0;

function measureVertical(el: HTMLElement) {
  const marginTop = parseFloat(getComputedStyle(el).marginTop);
  return el.getBoundingClientRect().height + (Number.isFinite(marginTop) ? marginTop : 0);
}

function updatePageSize() {
  const nav = listRef.value?.$el as HTMLElement | undefined;
  const page = nav?.closest(".start-page") as HTMLElement | null;
  const sample = nav?.querySelector<HTMLElement>(".start-item");
  // 没有剧本项时第一个 .start-item 落在分页行内，不拿它测量
  if (!nav || !page || !sample || sample.closest(".pagination-row")) return;

  const pageStyle = getComputedStyle(page);
  const available =
    page.getBoundingClientRect().height -
    parseFloat(pageStyle.borderTopWidth) -
    parseFloat(pageStyle.borderBottomWidth) -
    parseFloat(pageStyle.paddingTop) -
    parseFloat(pageStyle.paddingBottom);
  const rowUnit = measureVertical(sample);
  const pagination = nav.querySelector<HTMLElement>(".pagination-row");
  const paginationUnit = pagination ? measureVertical(pagination) : rowUnit;

  pageSize.value = Math.max(1, Math.floor((available - paginationUnit) / rowUnit));
}

/* 等布局结算后再算，避免 resize 瞬间读到旧字号 */
function onWindowResize() {
  window.clearTimeout(resizeTimer);
  resizeTimer = window.setTimeout(updatePageSize, 50);
}

watch(totalPages, () => {
  const maxPage = Math.max(1, totalPages.value);
  currentPage.value = Math.min(Math.max(1, currentPage.value), maxPage);
});

function prevPage() {
  if (currentPage.value > 1) currentPage.value--;
}

function nextPage() {
  if (currentPage.value < totalPages.value) currentPage.value++;
}

onMounted(() => {
  updatePageSize();
  window.addEventListener("resize", onWindowResize);
  // 字体加载完成后行高会变，补测一次；组件已卸载时 listRef 为空，回调会自行返回
  document.fonts?.ready.then(updatePageSize);
  currentPage.value = Math.min(Math.max(1, currentPage.value), Math.max(1, totalPages.value));
});

onUnmounted(() => {
  window.removeEventListener("resize", onWindowResize);
  window.clearTimeout(resizeTimer);
});
</script>

<style scoped>
/* 分页行常压在亮色屋顶/云上，加强描边和投影保证可读 */
.pagination-row :deep(.start-item) {
  -webkit-text-stroke: 0.024em rgba(30, 60, 100, 0.5);
  text-shadow: 0 2px 6px rgba(0, 0, 0, 0.6);
}
</style>

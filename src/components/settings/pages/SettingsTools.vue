<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="relative min-h-0 flex-1 overflow-hidden">
      <div class="custom-scrollbar absolute inset-0 overflow-y-auto px-1 pb-8">
        <!-- Android 说明 -->
        <div
          v-if="android"
          class="mb-4 rounded-xl border border-sky-300/30 bg-sky-400/10 px-4 py-3 text-sm"
        >
          <p class="font-semibold text-sky-200">{{ $t("ui.toolCalls.androidTitle") }}</p>
          <p class="mt-1 text-gray-300">{{ $t("ui.toolCalls.androidSummary") }}</p>
          <p v-if="runtimeInfo && !runtimeInfo.modelConfigured" class="mt-2 text-amber-300">
            {{ $t("ui.toolCalls.androidNoModel") }}
          </p>
          <p
            v-else-if="runtimeInfo && !runtimeInfo.nativeToolCallsSupported"
            class="mt-2 text-amber-300"
          >
            {{ $t("ui.toolCalls.androidModelUnsupported") }}
          </p>
          <p
            v-else-if="runtimeInfo && runtimeInfo.allowedTools.length === 0"
            class="mt-2 text-amber-300"
          >
            {{ $t("ui.toolCalls.androidNoTools") }}
          </p>
          <p v-else-if="runtimeInfo" class="mt-2 text-emerald-300">
            {{ $t("ui.toolCalls.androidReady", { count: runtimeInfo.allowedTools.length }) }}
          </p>
          <button
            type="button"
            class="mt-3 rounded-lg border border-sky-200/30 bg-sky-300/15 px-3 py-2 text-sky-100 transition-colors hover:bg-sky-300/25"
            @click="enableAndroidRecommended"
          >
            {{ $t("ui.toolCalls.androidEnableRecommended") }}
          </button>
        </div>

        <!-- 搜索与全局操作 -->
        <div class="flex flex-col gap-2 px-1 pb-3 sm:flex-row sm:items-center">
          <div
            class="flex min-w-0 flex-1 items-center gap-2 rounded-lg border border-white/15 bg-black/20 px-3 py-2 focus-within:border-white/35"
          >
            <input
              v-model="searchQuery"
              type="text"
              :placeholder="$t('ui.toolCalls.packageSearchPlaceholder')"
              class="min-w-0 flex-1 bg-transparent text-sm text-white outline-none placeholder:text-gray-500"
            />
            <button
              v-if="searchQuery"
              type="button"
              class="shrink-0 text-xs text-gray-400 transition-colors hover:text-white"
              :aria-label="$t('ui.toolCalls.clearSearch')"
              @click="searchQuery = ''"
            >
              ×
            </button>
          </div>
          <div class="flex items-center justify-between gap-2 sm:justify-end">
            <span class="text-xs text-gray-300">
              {{ $t("ui.toolCalls.enabledTotal", { count: store.totalEnabled }) }}
            </span>
            <button
              type="button"
              class="rounded-lg border border-red-400/40 bg-red-400/10 px-3 py-1.5 text-xs text-red-200 transition-colors duration-150 hover:bg-red-400/20"
              @click="disableAll"
            >
              {{ $t("ui.toolCalls.disableAll") }}
            </button>
          </div>
        </div>

        <!-- 工具包卡片 -->
        <div v-if="filteredPackages.length > 0" class="flex flex-col gap-2.5">
          <ToolPackCard
            v-for="pack in filteredPackages"
            :key="pack.group"
            :group="pack.group"
            :title="groupTitle(pack.group)"
            :help="groupHelp(pack.group)"
            :tools="toolsFor(pack.group)"
            :supported="pack.supported"
            :expanded="expandedGroups.includes(pack.group)"
            :dangerous="DANGEROUS_GROUPS.has(pack.group)"
            :has-settings="PACKS_WITH_SETTINGS.has(pack.group)"
            :access-mode="store.settings.access_mode"
            :query="searchQuery"
            @toggle-expand="toggleExpand(pack.group)"
            @toggle-package="(enabled: boolean) => store.setPackage(pack.group, enabled)"
            @toggle-tool="
              (tool: string, enabled: boolean) => store.setTool(pack.group, tool, enabled)
            "
            @open-settings="openSubpage(pack.group)"
            @reset="resetPackage(pack.group)"
          />
        </div>
        <div
          v-else-if="!store.loading"
          class="rounded-xl border border-dashed border-white/15 px-4 py-8 text-center"
        >
          <p class="text-sm text-gray-400">{{ $t("ui.toolCalls.searchEmpty") }}</p>
        </div>

        <!-- 其它高级设置 -->
        <button
          v-if="!searchQuery.trim()"
          type="button"
          class="mt-4 flex w-full items-center justify-between gap-3 rounded-xl border border-white/15 bg-white/5 px-4 py-4 text-left transition-colors duration-200 hover:border-white/30 hover:bg-white/10"
          @click="openSubpage('general')"
        >
          <span class="min-w-0">
            <span class="block font-semibold text-white">
              {{ $t("ui.toolCalls.globalSettingsTitle") }}
            </span>
            <span class="mt-1 block text-xs text-gray-400">
              {{ $t("ui.toolCalls.globalSettingsHint") }}
            </span>
          </span>
          <span class="shrink-0 text-lg text-gray-400" aria-hidden="true">›</span>
        </button>
      </div>

      <!-- 覆盖式子页 -->
      <Transition name="tool-subpage">
        <div
          v-if="activeSubpage && activeDraft"
          class="absolute inset-0 z-10 flex flex-col bg-black/40 p-2 backdrop-blur-xl"
        >
          <ToolPackSubpage
            :title="activeTitle"
            :show-reset="activeSubpage !== 'general'"
            @back="closeSubpage"
            @save="saveActive"
            @reset="resetActive"
          >
            <component :is="activePanel" v-if="activePanel" v-model="activeDraft" />
          </ToolPackSubpage>
        </div>
      </Transition>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch, type Component } from "vue";
import { useI18n } from "vue-i18n";
import {
  toolDisplayName,
  type ToolGroupCatalog,
  type ToolSettings,
} from "@/api/services/tool-settings";
import { useToolSettingsStore } from "@/stores/modules/settings/tools";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import { isAndroid } from "@/utils/platform";
import ToolPackCard from "@/components/settings/tools/ToolPackCard.vue";
import ToolPackSubpage from "@/components/settings/tools/ToolPackSubpage.vue";
import WebSearchPanel from "@/components/settings/tools/panels/WebSearchPanel.vue";
import MemoryPanel from "@/components/settings/tools/panels/MemoryPanel.vue";
import MediaPanel from "@/components/settings/tools/panels/MediaPanel.vue";
import GeneralPanel from "@/components/settings/tools/panels/GeneralPanel.vue";

const { t, te } = useI18n();
const store = useToolSettingsStore();
const dialogStore = useDialogStore();
const android = isAndroid();
const runtimeInfo = computed(() => store.runtimeInfo);

const DANGEROUS_GROUPS = new Set(["file_ops", "command"]);
const PACKS_WITH_SETTINGS = new Set(["web_search", "memory", "media"]);
const PANELS: Record<string, Component> = {
  web_search: WebSearchPanel,
  memory: MemoryPanel,
  media: MediaPanel,
  general: GeneralPanel,
};

const searchQuery = ref("");
const expandedGroups = ref<string[]>([]);
const activeSubpage = ref<string | null>(null);

const packages = computed(() => store.orderedPackages);
const filteredPackages = computed(() => packages.value.filter((pack) => matchesSearch(pack)));

const activeDraft = computed<ToolSettings | null>({
  get: () => (activeSubpage.value ? (store.drafts[activeSubpage.value] ?? null) : null),
  set: (value) => {
    if (activeSubpage.value && value) store.drafts[activeSubpage.value] = value;
  },
});

const activePanel = computed<Component | null>(() =>
  activeSubpage.value ? (PANELS[activeSubpage.value] ?? null) : null,
);

const activeTitle = computed(() => {
  if (!activeSubpage.value) return "";
  return activeSubpage.value === "general"
    ? t("ui.toolCalls.globalSettingsTitle")
    : groupTitle(activeSubpage.value);
});

function groupTitle(group: string) {
  if (group === "web_search") return t("ui.toolCalls.webSearchTitle");
  if (group === "plugin") return t("ui.toolCalls.pluginPackageTitle");
  const key = `ui.toolCalls.nav.${group}`;
  return te(key) ? t(key) : t(`ui.toolCalls.groups.${group}`);
}

function groupHelp(group: string) {
  const key = `ui.toolCalls.groups.${group}`;
  return te(key) ? t(key) : "";
}

function toolsFor(group: string) {
  return store.packageTools(group).map((name) => ({
    name,
    checked: store.isToolChecked(group, name),
  }));
}

function matchesSearch(pack: ToolGroupCatalog) {
  const query = searchQuery.value.trim().toLowerCase();
  if (!query) return true;
  if (groupTitle(pack.group).toLowerCase().includes(query)) return true;
  return pack.tools.some(
    (tool) =>
      tool.toLowerCase().includes(query) || toolDisplayName(tool).toLowerCase().includes(query),
  );
}

function toggleExpand(group: string) {
  const index = expandedGroups.value.indexOf(group);
  if (index === -1) {
    expandedGroups.value.push(group);
  } else {
    expandedGroups.value.splice(index, 1);
  }
}

// 搜索命中工具时自动展开所属工具包
watch(searchQuery, (value) => {
  const query = value.trim().toLowerCase();
  if (!query) return;
  for (const pack of packages.value) {
    const hit = pack.tools.some(
      (tool) =>
        tool.toLowerCase().includes(query) || toolDisplayName(tool).toLowerCase().includes(query),
    );
    if (hit && !expandedGroups.value.includes(pack.group)) {
      expandedGroups.value.push(pack.group);
    }
  }
});

function enableAndroidRecommended() {
  store.enableAndroidRecommended();
}

async function disableAll() {
  const approved = await dialogStore.confirm(
    t("ui.toolCalls.disableAllConfirmMessage"),
    t("ui.toolCalls.disableAllConfirmTitle"),
  );
  if (!approved) return;
  store.disableAll();
}

async function resetPackage(group: string) {
  const approved = await dialogStore.confirm(
    t("ui.toolCalls.resetPackageConfirmMessage"),
    t("ui.toolCalls.resetPackageConfirmTitle"),
  );
  if (!approved) return;
  store.resetPackage(group);
}

function openSubpage(group: string) {
  store.openDraft(group);
  activeSubpage.value = group;
}

function closeSubpage() {
  if (activeSubpage.value) store.closeDraft(activeSubpage.value);
  activeSubpage.value = null;
}

async function saveActive() {
  if (!activeSubpage.value) return;
  const group = activeSubpage.value;
  await store.saveDraft(group);
  // 保存时前端会夹取数值范围，刷新草稿让界面与落盘值一致。
  store.openDraft(group);
}

async function resetActive() {
  if (!activeSubpage.value) return;
  const approved = await dialogStore.confirm(
    t("ui.toolCalls.resetPackageConfirmMessage"),
    t("ui.toolCalls.resetPackageConfirmTitle"),
  );
  if (!approved) return;
  store.resetPackage(activeSubpage.value);
  store.openDraft(activeSubpage.value);
}

onMounted(() => {
  void store.load();
});
</script>

<style scoped>
.tool-subpage-enter-active,
.tool-subpage-leave-active {
  transition:
    transform 0.2s ease-out,
    opacity 0.2s ease-out;
}
.tool-subpage-enter-from,
.tool-subpage-leave-to {
  transform: translateX(100%);
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .tool-subpage-enter-active,
  .tool-subpage-leave-active {
    transition-duration: 0.01ms;
  }
}
</style>

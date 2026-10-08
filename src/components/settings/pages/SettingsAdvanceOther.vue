<template>
  <div class="flex h-full min-h-0 flex-col md:grid md:grid-cols-[min(30%,280px)_1fr]">
    <!-- 导航菜单：宽屏始终可见；窄屏仅在浏览菜单层级时可见 -->
    <nav
      ref="navContainerRef"
      v-show="!uiStore.isNarrowScreen || narrowViewLevel === 'menu'"
      @click="() => removeMoreMenu()"
      class="border-brand md:moreMenu:left-0 relative flex flex-col justify-start gap-6.25 overflow-y-auto border-b transition-all duration-300 ease-[cubic-bezier(0.18,0.89,0.32,1.00)] md:border-r md:border-b-0"
      :class="['md:left-0', 'translate-y-0', 'moreMenu:translate-y-0']"
    >
      <!-- 滑动指示器 -->
      <div
        ref="indicatorRef"
        class="bg-brand absolute left-2 z-0 w-[calc(100%-40px)] rounded-lg transition-all duration-300 ease-[cubic-bezier(0.18,0.89,0.32,1.00)]"
      ></div>

      <div
        class="mt-2 flex items-center gap-1 px-5 text-sm"
        style="color: white; -webkit-text-stroke: 1px black; paint-order: stroke fill"
      >
        {{ $t("settings.advanceOther.restartHint") }}
      </div>

      <div
        v-for="(categoryData, categoryName) in configData"
        :key="categoryName"
        class="flex w-full flex-col gap-1"
      >
        <span
          class="text-brand mb-1 block rounded-lg border border-white/10 bg-white/10 px-3.75 py-2.5 text-base font-bold shadow-[0_8px_32px_rgba(0,0,0,0.1),inset_0_1px_1px_rgba(255,255,255,0.1)]"
          >{{ catLabel(categoryName) }}</span
        >
        <a
          v-for="(, subcategoryName) in categoryData.subcategories"
          :key="subcategoryName"
          href="#"
          class="adv-nav-link relative z-10 block rounded-lg px-5 py-3 text-white no-underline transition-colors duration-200 hover:bg-gray-200 hover:text-black active:font-bold active:text-white"
          :class="{
            active: isActive(categoryName, subcategoryName.toString()),
          }"
          @click.prevent="selectSubcategory(categoryName, subcategoryName.toString())"
        >
          {{ subLabel(subcategoryName) }}
        </a>
      </div>
    </nav>

    <!-- 设置内容区域：宽屏始终可见；窄屏仅在浏览内容层级时可见 -->
    <main
      v-show="!uiStore.isNarrowScreen || narrowViewLevel === 'content'"
      class="relative flex h-full justify-center overflow-auto px-10 py-10 md:px-10 md:py-0"
      :class="['translate-y-0', 'moreMenu:translate-y-0']"
    >
      <!-- 窄屏返回按钮 -->
      <button
        v-if="uiStore.isNarrowScreen"
        class="absolute top-0 left-4 flex items-center gap-1.5 rounded-lg px-2 py-1 text-sm text-white/70 transition-colors hover:bg-white/10 hover:text-white"
        @click="narrowViewLevel = 'menu'"
      >
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M15 19l-7-7 7-7"
          />
        </svg>
        {{ $t("settings.advanceOther.backToList") }}
      </button>
      <div v-if="selectedSubcategory" class="active w-full">
        <div class="overflow-auto pt-2.5">
          <header class="border-brand mb-6 border-b pb-4">
            <h2 class="text-brand m-0 text-2xl font-semibold">
              {{ subLabel(activeSelection.subcategory ?? "") }}
            </h2>
            <p class="mt-2 text-base">
              {{
                subDesc(activeSelection.subcategory ?? "", selectedSubcategory.description) ||
                $t("settings.advanceOther.subcategoryDesc", {
                  name: subLabel(activeSelection.subcategory ?? ""),
                })
              }}
            </p>
          </header>

          <form @submit.prevent="saveSettings">
            <div v-for="setting in selectedSubcategory.settings" :key="setting.key" class="mb-6">
              <SettingItem
                :setting="localizedSetting(setting)"
                @update:value="(value) => (setting.value = value)"
              />
            </div>
          </form>

          <section
            v-if="activeSelection.category === 'TTS 配置'"
            class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4"
          >
            <h3 class="mb-1 text-base font-semibold text-white">
              {{ $t("settings.advanceOther.ttsControl.title") }}
            </h3>
            <p class="mb-3 text-sm leading-6 text-white/65">
              {{ $t("settings.advanceOther.ttsControl.desc") }}
            </p>
            <Button type="big" :disabled="isReconnectingTts" @click="forceReconnectTts">
              <RefreshCw :size="18" :class="{ 'animate-spin': isReconnectingTts }" />
              {{
                isReconnectingTts
                  ? $t("settings.advanceOther.ttsControl.reconnecting")
                  : $t("settings.advanceOther.ttsControl.forceReconnect")
              }}
            </Button>
            <p
              v-if="reconnectStatus.message"
              class="mt-2 text-sm"
              :class="reconnectStatus.colorClass"
            >
              {{ reconnectStatus.message }}
            </p>
          </section>

          <!-- 好感度动画开关：前端即时生效，不走后端配置树 -->
          <section
            v-if="activeSelection.category === '好感度' && activeSelection.subcategory === '通用'"
            class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4"
          >
            <h3 class="mb-1 text-base font-semibold text-white">
              {{ $t("settings.advanceOther.affectionAnim.title") }}
            </h3>
            <p class="mb-3 text-sm leading-6 text-white/65">
              {{ $t("settings.advanceOther.affectionAnim.desc") }}
            </p>
            <Toggle
              :checked="affectionHeartbeatEnabled"
              @change="settingsStore.setAffectionHeartbeatEnabled($event)"
            >
              {{ $t("settings.advanceOther.affectionAnim.heartbeat") }}
            </Toggle>
            <Toggle
              class="mt-2"
              :checked="affectionWaveEnabled"
              @change="settingsStore.setAffectionWaveEnabled($event)"
            >
              {{ $t("settings.advanceOther.affectionAnim.wave") }}
            </Toggle>
          </section>

          <!-- 长期记忆浏览：查看/删除各角色已存的向量记忆，不走后端配置树 -->
          <section
            v-if="activeSelection.subcategory === '记忆系统'"
            class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4"
          >
            <h3 class="mb-1 text-base font-semibold text-white">
              {{ $t("settings.advanceOther.memoryViewer.title") }}
            </h3>
            <p class="mb-3 text-sm leading-6 text-white/65">
              {{ $t("settings.advanceOther.memoryViewer.desc") }}
            </p>
            <Button type="big" :disabled="memoryLoading" @click="toggleMemoryViewer">
              <RefreshCw :size="18" :class="{ 'animate-spin': memoryLoading }" />
              {{
                memoryViewerOpen
                  ? $t("settings.advanceOther.memoryViewer.hideButton")
                  : $t("settings.advanceOther.memoryViewer.viewButton")
              }}
            </Button>

            <div v-if="memoryViewerOpen" class="mt-4">
              <p v-if="memoryError" class="mb-2 text-sm text-red-400">{{ memoryError }}</p>

              <div v-if="!memoryLoading && !memoryRoles.length" class="text-sm text-white/60">
                {{ $t("settings.advanceOther.memoryViewer.emptyRoles") }}
              </div>

              <template v-if="memoryRoles.length">
                <div class="mb-2 flex flex-wrap items-center gap-2">
                  <select
                    v-model="memoryRoleId"
                    class="rounded-lg border border-white/15 bg-black/40 px-3 py-1.5 text-sm text-white outline-none"
                    @change="loadMemoryList"
                  >
                    <option v-for="role in memoryRoles" :key="role.role_id" :value="role.role_id">
                      {{ memoryRoleLabel(role) }}（{{ role.count }}）
                    </option>
                  </select>
                  <button
                    class="cursor-pointer rounded-lg border border-white/15 bg-white/10 px-3 py-1.5 text-sm text-white transition-colors hover:bg-white/20 disabled:opacity-60"
                    :disabled="memoryLoading"
                    @click="refreshMemoryViewer"
                  >
                    {{ $t("settings.advanceOther.memoryViewer.refresh") }}
                  </button>
                </div>

                <p v-if="memoryLoading" class="text-sm text-white/60">
                  {{ $t("settings.advanceOther.memoryViewer.loading") }}
                </p>
                <p v-else-if="!memories.length" class="text-sm text-white/60">
                  {{ $t("settings.advanceOther.memoryViewer.emptyList") }}
                </p>
                <ul v-else class="m-0 flex list-none flex-col gap-2 p-0">
                  <li
                    v-for="item in memories"
                    :key="item.memory_id"
                    class="rounded-lg border border-white/10 bg-black/25 p-3"
                  >
                    <div class="mb-1 flex items-center justify-between gap-2">
                      <span class="text-xs text-white/50">{{
                        formatMemoryTime(item.created_at)
                      }}</span>
                      <button
                        class="cursor-pointer border-none bg-transparent text-xs text-red-400 transition-colors hover:text-red-300 disabled:opacity-60"
                        :disabled="deletingMemoryId === item.memory_id"
                        @click="removeMemory(item)"
                      >
                        {{
                          deletingMemoryId === item.memory_id
                            ? $t("settings.advanceOther.memoryViewer.deleting")
                            : $t("settings.advanceOther.memoryViewer.delete")
                        }}
                      </button>
                    </div>
                    <p class="m-0 text-sm leading-6 break-words whitespace-pre-wrap text-white/90">
                      {{ item.content }}
                    </p>
                    <p v-if="item.tags.length" class="mt-1 mb-0 text-xs text-white/45">
                      {{ $t("settings.advanceOther.memoryViewer.tags") }}{{ item.tags.join(", ") }}
                    </p>
                  </li>
                </ul>
              </template>
            </div>
          </section>

          <!-- 旧版存档记忆整合：一键导入 + 浏览/删除独立归档表 -->
          <section
            v-if="activeSelection.subcategory === '记忆系统'"
            class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4"
          >
            <h3 class="mb-1 text-base font-semibold text-white">
              {{ $t("settings.advanceOther.legacyMemory.title") }}
            </h3>
            <p class="mb-3 text-sm leading-6 text-white/65">
              {{ $t("settings.advanceOther.legacyMemory.desc") }}
            </p>
            <div class="flex flex-wrap items-center gap-2">
              <Button type="big" :disabled="legacyImporting" @click="runLegacyImport">
                <RefreshCw :size="18" :class="{ 'animate-spin': legacyImporting }" />
                {{
                  legacyImporting
                    ? $t("settings.advanceOther.legacyMemory.importing")
                    : $t("settings.advanceOther.legacyMemory.importButton")
                }}
              </Button>
              <button
                v-if="legacyArchives.length"
                class="cursor-pointer rounded-lg border border-white/15 bg-white/10 px-3 py-1.5 text-sm text-white transition-colors hover:bg-white/20 disabled:opacity-60"
                :disabled="legacyLoading"
                @click="refreshLegacyArchives"
              >
                {{ $t("settings.advanceOther.memoryViewer.refresh") }}
              </button>
            </div>

            <p v-if="legacyError" class="mt-3 mb-0 text-sm text-red-400">{{ legacyError }}</p>

            <div v-if="legacyLoading" class="mt-3 text-sm text-white/60">
              {{ $t("settings.advanceOther.memoryViewer.loading") }}
            </div>

            <div v-else-if="legacyArchives.length" class="mt-3 flex flex-col gap-2">
              <div
                v-for="archive in legacyArchives"
                :key="archive.id"
                class="rounded-lg border border-white/10 bg-black/25 p-3"
              >
                <div class="mb-1 flex flex-wrap items-center justify-between gap-2">
                  <div class="min-w-0">
                    <span class="text-sm font-medium break-words text-white">
                      {{ archive.role_name }}
                    </span>
                    <span class="ml-2 text-xs text-white/50">
                      {{
                        archive.script_key
                          ? archive.script_key
                          : $t("settings.advanceOther.legacyMemory.globalScript")
                      }}{{ archive.script_role_key ? " / " + archive.script_role_key : "" }}
                    </span>
                  </div>
                  <div class="flex flex-none items-center gap-2 text-xs">
                    <span class="text-white/50">{{ archive.entry_count }}</span>
                    <button
                      class="cursor-pointer rounded-md border border-white/15 bg-white/10 px-2 py-1 text-xs text-white transition-colors hover:bg-white/20 disabled:opacity-60"
                      :disabled="legacyDeletingId === archive.id"
                      @click="toggleLegacyArchive(archive)"
                    >
                      {{
                        legacyEntries[archive.id] !== undefined
                          ? $t("settings.advanceOther.legacyMemory.hide")
                          : $t("settings.advanceOther.legacyMemory.view")
                      }}
                    </button>
                    <button
                      class="cursor-pointer rounded-md border border-red-400/30 bg-red-400/10 px-2 py-1 text-xs text-red-400 transition-colors hover:bg-red-400/20 disabled:opacity-60"
                      :disabled="legacyDeletingId === archive.id"
                      @click="removeLegacyArchive(archive)"
                    >
                      {{
                        legacyDeletingId === archive.id
                          ? $t("settings.advanceOther.legacyMemory.deleting")
                          : $t("settings.advanceOther.legacyMemory.delete")
                      }}
                    </button>
                  </div>
                </div>

                <div v-if="legacyEntries[archive.id]" class="mt-2 flex flex-col gap-2">
                  <p v-if="!legacyEntries[archive.id].length" class="m-0 text-sm text-white/60">
                    {{ $t("settings.advanceOther.legacyMemory.emptyEntry") }}
                  </p>
                  <div
                    v-for="entry in legacyEntries[archive.id]"
                    :key="entry.id"
                    class="rounded-md border border-white/10 bg-black/20 p-2"
                  >
                    <span class="mb-1 block text-xs text-white/50">
                      {{ sectionLabel(entry.section) }}
                    </span>
                    <p class="m-0 text-sm leading-6 break-words whitespace-pre-wrap text-white/90">
                      {{ entry.content }}
                    </p>
                  </div>
                </div>
              </div>
            </div>

            <div v-else-if="legacyLoaded" class="mt-3 text-sm text-white/60">
              {{ $t("settings.advanceOther.legacyMemory.empty") }}
            </div>
          </section>

          <!-- 保存操作区域 -->
          <div
            class="bg-brand inline-flex min-w-30 cursor-pointer flex-col gap-2 rounded-lg border-none px-5 py-2.5 text-sm font-medium text-white transition-colors duration-200 hover:bg-[#0056b3]"
            @click="saveSettings"
          >
            <button
              class="m-0 h-full w-full cursor-pointer border-none bg-transparent p-0 text-white"
            >
              {{ $t("settings.advanceOther.saveButton") }}
            </button>
            <p
              :class="saveStatus.colorClass"
              class="max-w-75 text-xs wrap-break-word whitespace-normal"
            >
              {{ saveStatus.message }}
            </p>
          </div>
        </div>
      </div>
      <div v-else-if="!isLoading && !Object.keys(configData).length" class="active w-full">
        <div class="advanced-settings-container">
          <header>
            <h2 class="adv-title">{{ $t("settings.advanceOther.loadFailed") }}</h2>
            <p class="adv-description">{{ $t("settings.advanceOther.loadFailedDesc") }}</p>
          </header>
        </div>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, reactive, watch, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useSettingsStore } from "@/stores/modules/settings";
import SettingItem from "@/components/base/items/SettingItem.vue";
import { Button, Toggle } from "@/components/base";
import { getEnvConfigSettings, saveEnvConfigSettings } from "@/api/services/config";
import { reactivateTTS } from "@/api/services/game-info";
import { switchLlm } from "@/api/services/llm-providers";
import {
  deleteMemory,
  deleteMemoryArchive,
  getMemoryArchive,
  importLegacyMemories,
  listMemories,
  listMemoryArchives,
  listMemoryRoles,
  type LegacyImportResult,
  type MemoryArchiveEntry,
  type MemoryArchiveSummary,
  type MemoryItem,
  type MemoryRoleSummary,
} from "@/api/services/memory";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import { RefreshCw } from "lucide-vue-next";

// --- 响应式状态定义 ---
const uiStore = useUIStore();
const settingsStore = useSettingsStore();
const dialogStore = useDialogStore();
const { t, te } = useI18n();

// 好感度动画开关（前端设置项，即时生效，不入后端配置树）
const affectionHeartbeatEnabled = computed(() => settingsStore.affectionHeartbeatEnabled);
const affectionWaveEnabled = computed(() => settingsStore.affectionWaveEnabled);

// 后端配置树的分类/子类/设置项描述均为中文（config/tree.rs），
// 这里按名称/键查 i18n 词条做界面日文化；查不到时回退后端原文。
const catLabel = (name: string) =>
  te(`settings.advanceOther.categories.${name}`)
    ? t(`settings.advanceOther.categories.${name}`)
    : name;
const subLabel = (name: string) =>
  te(`settings.advanceOther.subcategories.${name}`)
    ? t(`settings.advanceOther.subcategories.${name}`)
    : name;
const subDesc = (name: string, fallback: string) =>
  te(`settings.advanceOther.subcategoryDescs.${name}`)
    ? t(`settings.advanceOther.subcategoryDescs.${name}`)
    : fallback;
const localizedSetting = (setting: any) => ({
  ...setting,
  description: te(`settings.advanceOther.fields.${setting.key}`)
    ? t(`settings.advanceOther.fields.${setting.key}`)
    : setting.description,
});
const narrowViewLevel = ref<"menu" | "content">("menu");
const isLoading = ref(false);
const configData = ref<Record<string, any>>({});
const activeSelection = reactive({
  category: null as string | null,
  subcategory: null as string | null,
});
const saveStatus = reactive({
  message: "",
  colorClass: "text-green-500",
});
const isReconnectingTts = ref(false);
const reconnectStatus = reactive({
  message: "",
  colorClass: "text-green-400",
});
let reconnectStatusTimer: ReturnType<typeof setTimeout> | null = null;

const emit = defineEmits<{
  "remove-more-menu-from-b": [];
}>();

// --- Refs for DOM elements ---
const navContainerRef = ref<HTMLElement | null>(null);
const indicatorRef = ref<HTMLElement | null>(null);

// --- 计算属性 ---
const selectedSubcategory = computed(() => {
  if (activeSelection.category && activeSelection.subcategory) {
    return configData.value[activeSelection.category]?.subcategories[activeSelection.subcategory];
  }
  return null;
});

// --- 方法定义 ---

const isActive = (category: string, subcategory: string) => {
  return activeSelection.category === category && activeSelection.subcategory === subcategory;
};

const selectSubcategory = (category: string, subcategory: string) => {
  activeSelection.category = category;
  activeSelection.subcategory = subcategory;
  // 窄屏下自动切换到内容视图
  if (uiStore.isNarrowScreen) {
    narrowViewLevel.value = "content";
  }
};

const saveSettings = async () => {
  if (!selectedSubcategory.value) return;

  const formData: Record<string, string> = {};
  selectedSubcategory.value.settings.forEach((setting: { key: string; value: string }) => {
    formData[setting.key] = setting.value;
  });

  isLoading.value = true;
  saveStatus.message = "";

  try {
    saveStatus.message = (await saveEnvConfigSettings(formData)).message;
    if (Object.prototype.hasOwnProperty.call(formData, "llm.timeout_secs")) {
      await switchLlm();
    }
    saveStatus.colorClass = "text-green-500";

    await loadConfig(false);
  } catch (error: any) {
    saveStatus.message = t("settings.advanceOther.msg.error", { error: error.message });
    saveStatus.colorClass = "text-red-500";
  } finally {
    isLoading.value = false;
    setTimeout(() => {
      saveStatus.message = "";
    }, 5000);
  }
};

const forceReconnectTts = async () => {
  if (isReconnectingTts.value) return;

  isReconnectingTts.value = true;
  reconnectStatus.message = t("settings.advanceOther.msg.ttsReactivating");
  reconnectStatus.colorClass = "text-white/70";
  if (reconnectStatusTimer) {
    clearTimeout(reconnectStatusTimer);
    reconnectStatusTimer = null;
  }

  try {
    await reactivateTTS();
    reconnectStatus.message = t("settings.advanceOther.msg.ttsReactivated");
    reconnectStatus.colorClass = "text-green-400";
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    reconnectStatus.message = t("settings.advanceOther.msg.ttsReconnectFailed", {
      error: message,
    });
    reconnectStatus.colorClass = "text-red-400";
  } finally {
    isReconnectingTts.value = false;
    reconnectStatusTimer = setTimeout(() => {
      reconnectStatus.message = "";
      reconnectStatusTimer = null;
    }, 8000);
  }
};

// --- 长期记忆浏览面板（设置页「记忆系统」子页） ---
const memoryViewerOpen = ref(false);
const memoryLoading = ref(false);
const memoryError = ref("");
const memoryRoles = ref<MemoryRoleSummary[]>([]);
const memoryRoleId = ref<number | null>(null);
const memories = ref<MemoryItem[]>([]);
const deletingMemoryId = ref<string | null>(null);

const memoryRoleLabel = (role: MemoryRoleSummary) =>
  role.name || t("settings.advanceOther.memoryViewer.unknownRole", { id: role.role_id });

const formatMemoryTime = (timestamp: number) => new Date(timestamp).toLocaleString();

const memoryErrorText = (error: unknown) =>
  t("settings.advanceOther.memoryViewer.loadFailed", {
    error: error instanceof Error ? error.message : String(error),
  });

const loadMemoryList = async () => {
  if (memoryRoleId.value === null || memoryLoading.value) return;
  memoryLoading.value = true;
  memoryError.value = "";
  try {
    memories.value = await listMemories(memoryRoleId.value);
  } catch (error) {
    memories.value = [];
    memoryError.value = memoryErrorText(error);
  } finally {
    memoryLoading.value = false;
  }
};

/** 拉取「有记忆的角色」清单并刷新当前角色的列表（首次打开 / 切换后刷新 / 删除后重数）。 */
const refreshMemoryViewer = async () => {
  if (memoryLoading.value) return;
  memoryLoading.value = true;
  memoryError.value = "";
  try {
    memoryRoles.value = await listMemoryRoles();
    if (!memoryRoles.value.some((role) => role.role_id === memoryRoleId.value)) {
      memoryRoleId.value = memoryRoles.value[0]?.role_id ?? null;
    }
    memories.value = memoryRoleId.value === null ? [] : await listMemories(memoryRoleId.value);
  } catch (error) {
    memoryError.value = memoryErrorText(error);
  } finally {
    memoryLoading.value = false;
  }
};

const toggleMemoryViewer = async () => {
  if (memoryViewerOpen.value) {
    memoryViewerOpen.value = false;
    return;
  }
  memoryViewerOpen.value = true;
  await refreshMemoryViewer();
};

const removeMemory = async (item: MemoryItem) => {
  if (deletingMemoryId.value || memoryRoleId.value === null) return;
  const confirmed = await dialogStore.confirm(
    t("settings.advanceOther.memoryViewer.deleteConfirm"),
    t("settings.advanceOther.memoryViewer.deleteConfirmTitle"),
  );
  if (!confirmed) return;
  deletingMemoryId.value = item.memory_id;
  try {
    await deleteMemory(memoryRoleId.value, item.memory_id);
    await refreshMemoryViewer();
  } catch (error) {
    memoryError.value = memoryErrorText(error);
  } finally {
    deletingMemoryId.value = null;
  }
};

// --- 旧版存档记忆整合面板（每「剧本 × 角色」独立归档表） ---
const legacyImporting = ref(false);
const legacyLoading = ref(false);
const legacyLoaded = ref(false);
const legacyError = ref("");
const legacyArchives = ref<MemoryArchiveSummary[]>([]);
const legacyEntries = ref<Record<number, MemoryArchiveEntry[]>>({});
const legacyDeletingId = ref<number | null>(null);

const legacyErrorText = (error: unknown) =>
  t("settings.advanceOther.legacyMemory.loadFailed", {
    error: error instanceof Error ? error.message : String(error),
  });

const sectionLabel = (section: string) =>
  te(`settings.advanceOther.legacyMemory.sectionLabels.${section}`)
    ? t(`settings.advanceOther.legacyMemory.sectionLabels.${section}`)
    : section;

const refreshLegacyArchives = async () => {
  if (legacyLoading.value) return;
  legacyLoading.value = true;
  legacyError.value = "";
  try {
    legacyArchives.value = await listMemoryArchives();
    legacyLoaded.value = true;
  } catch (error) {
    legacyError.value = legacyErrorText(error);
  } finally {
    legacyLoading.value = false;
  }
};

const runLegacyImport = async () => {
  if (legacyImporting.value) return;
  const confirmed = await dialogStore.confirm(
    t("settings.advanceOther.legacyMemory.importConfirm"),
    t("settings.advanceOther.legacyMemory.importConfirmTitle"),
  );
  if (!confirmed) return;
  legacyImporting.value = true;
  legacyError.value = "";
  try {
    const result: LegacyImportResult = await importLegacyMemories();
    await refreshLegacyArchives();
    await dialogStore.alert(
      t("settings.advanceOther.legacyMemory.importSum", {
        legacyRows: result.legacy_rows,
        imported: result.imported_roles,
        memories: result.memories_written,
        skippedExisting: result.skipped_existing,
        skippedEmpty: result.skipped_empty,
      }),
      t("settings.advanceOther.legacyMemory.importTitle"),
    );
  } catch (error) {
    legacyError.value = legacyErrorText(error);
  } finally {
    legacyImporting.value = false;
  }
};

const toggleLegacyArchive = async (archive: MemoryArchiveSummary) => {
  if (legacyEntries.value[archive.id] !== undefined) {
    const next = { ...legacyEntries.value };
    delete next[archive.id];
    legacyEntries.value = next;
    return;
  }
  legacyError.value = "";
  try {
    const entries = await getMemoryArchive(archive.id);
    legacyEntries.value = { ...legacyEntries.value, [archive.id]: entries };
  } catch (error) {
    legacyError.value = legacyErrorText(error);
  }
};

const removeLegacyArchive = async (archive: MemoryArchiveSummary) => {
  if (legacyDeletingId.value !== null) return;
  const confirmed = await dialogStore.confirm(
    t("settings.advanceOther.legacyMemory.deleteConfirm"),
    t("settings.advanceOther.legacyMemory.deleteConfirmTitle"),
  );
  if (!confirmed) return;
  legacyDeletingId.value = archive.id;
  legacyError.value = "";
  try {
    await deleteMemoryArchive(archive.id);
    const next = { ...legacyEntries.value };
    delete next[archive.id];
    legacyEntries.value = next;
    await refreshLegacyArchives();
  } catch (error) {
    legacyError.value = legacyErrorText(error);
  } finally {
    legacyDeletingId.value = null;
  }
};

const loadConfig = async (selectFirst = true) => {
  isLoading.value = true;
  try {
    configData.value = await getEnvConfigSettings();

    if (selectFirst && Object.keys(configData.value).length > 0) {
      const firstCategory = Object.keys(configData.value)[0];
      if (firstCategory) {
        const firstSubcategory = Object.keys(
          configData.value[firstCategory]?.subcategories || {},
        )[0];

        if (firstCategory && firstSubcategory) {
          selectSubcategory(firstCategory, firstSubcategory);
        }
      }
    }
  } catch (error: any) {
    console.error(error);
    saveStatus.message = t("settings.advanceOther.msg.loadConfigFailed", {
      error: error.message,
    });
    saveStatus.colorClass = "text-red-500";
  } finally {
    isLoading.value = false;
  }
};

// --- 导航指示器逻辑 ---
const updateIndicatorPosition = () => {
  if (!navContainerRef.value || !indicatorRef.value) return;

  const activeLink = navContainerRef.value.querySelector(".adv-nav-link.active") as HTMLElement;

  if (activeLink) {
    const top = activeLink.offsetTop;
    const height = activeLink.offsetHeight;

    if (top) {
      indicatorRef.value.style.top = `${top}px`;
    }
    if (height) {
      indicatorRef.value.style.height = `${height}px`;
    }
  }
};

// --- 监听导航容器尺寸变化 ---
const setupNavResizeObserver = () => {
  if (!navContainerRef.value) return;

  const resizeObserver = new ResizeObserver(() => {
    updateIndicatorPosition();
  });

  resizeObserver.observe(navContainerRef.value);
};

// 监视 activeSelection 的变化，并在 DOM 更新后移动指示器
watch(
  activeSelection,
  async () => {
    await nextTick();
    updateIndicatorPosition();
  },
  { deep: true },
);

// --- 生命周期钩子 ---
onMounted(async () => {
  await loadConfig();
  await nextTick();
  updateIndicatorPosition();
  setupNavResizeObserver();
  refreshLegacyArchives();
});

onUnmounted(() => {
  if (reconnectStatusTimer) {
    clearTimeout(reconnectStatusTimer);
  }
});

// --- 窄屏菜单控制 ---
const addMoreMenu = () => {
  const btnEl = navContainerRef.value as HTMLElement | null;
  if (btnEl) {
    btnEl.classList.add("moreMenu");
  }
};

const removeMoreMenu = () => {
  const btnEl = navContainerRef.value as HTMLElement | null;
  if (btnEl) {
    btnEl.classList.remove("moreMenu");
  }
  emit("remove-more-menu-from-b");
};

defineExpose({
  addMoreMenu,
});
</script>

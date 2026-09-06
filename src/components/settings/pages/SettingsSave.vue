<template>
  <MenuPage>
    <MenuItem :title="$t('settings.save.create.title')">
      <template #header>
        <PencilLine :size="20" />
      </template>
      <div class="flex gap-2.5">
        <Input
          type="text"
          v-model="newSaveTitle"
          :placeholder="$t('settings.save.create.placeholder')"
          @keyup.enter="handleCreateSave"
        />
        <button
          class="w-[10%] min-w-[65px] cursor-pointer rounded-md border border-[rgba(0,255,55,0.3)]
            bg-[rgba(0,255,55,0.2)] px-4 py-2 whitespace-nowrap text-[#ddd] transition-all
            duration-200 hover:-translate-y-px hover:bg-[rgba(0,255,55,0.35)]
            hover:shadow-[0_0_10px_rgba(0,255,55,0.15)]"
          @click="handleCreateSave"
          :disabled="actionLoading !== null"
        >
          {{
            actionLoading === -1
              ? $t("settings.save.create.creating")
              : $t("settings.save.create.button")
          }}
        </button>
      </div>
    </MenuItem>
    <MenuItem :title="$t('settings.save.list.title')">
      <template #header>
        <LayoutList :size="20" />
      </template>
      <div class="flex flex-col">
        <div class="h-[calc(100dvh-22rem)] min-h-[300px] overflow-y-auto pr-1 pb-4">
          <div v-if="loading" class="p-8 text-center text-[#888]">
            {{ $t("settings.shared.loading") }}
          </div>

          <div v-else-if="error" class="p-8 text-center text-[#ff6b6b]">
            {{ $t("settings.save.list.loadFailed", { error }) }}
          </div>

          <div v-else-if="saves.length === 0" class="p-8 text-center text-[#888]">
            {{ $t("settings.save.list.empty") }}
          </div>

          <div v-else class="grid grid-cols-1 gap-5 p-[5px] md:grid-cols-2">
            <div
              v-for="(save, index) in saves"
              :key="save.id"
              class="flex flex-col rounded-xl border border-white/10 bg-[rgba(20,20,20,0.45)] p-4
                shadow-[0_8px_32px_rgba(0,0,0,0.2)] backdrop-blur-[10px] transition-all duration-300
                ease-[cubic-bezier(0.25,0.8,0.25,1)] hover:-translate-y-[3px]
                hover:border-[rgba(121,217,255,0.35)] hover:bg-[rgba(20,20,20,0.55)]
                hover:shadow-[0_12px_40px_rgba(121,217,255,0.08)]"
            >
              <div class="flex gap-4">
                <!-- Left: Screenshot Preview -->
                <div
                  class="h-48 w-1/2 shrink-0 overflow-hidden rounded-lg border border-white/[0.08]
                    bg-black/40"
                >
                  <img
                    v-if="save.screenshot"
                    :src="`${convertFileSrc(save.screenshot)}?v=${save.update_date}`"
                    class="h-full w-full animate-[fadeIn_0.4s_ease] object-cover"
                    alt="game screenshot"
                  />
                  <div
                    v-else
                    class="flex h-full w-full flex-col items-center justify-center bg-white/[0.02]"
                  >
                    <SaveIcon :size="24" class="mb-1 text-white/20" />
                    <span class="text-[10px] font-semibold text-white/30">{{
                      $t("settings.save.list.noScreenshot")
                    }}</span>
                  </div>
                </div>

                <!-- Right: Save Info -->
                <div class="flex flex-1 flex-col justify-between overflow-hidden">
                  <!-- Line 1: Index & Time -->
                  <div class="flex items-center justify-between font-mono text-xs text-white/40">
                    <span class="font-bold">No.{{ index + 1 }}</span>
                    <span class="flex items-center gap-1">
                      <Clock :size="10" />
                      {{ formatDate(save.update_date) }}
                    </span>
                  </div>

                  <!-- Line 2: Title (Editable on Double Click) -->
                  <div class="mt-1.5 flex min-h-[26px] items-center">
                    <input
                      v-if="editingSaveId === save.id"
                      v-model="editTitleText"
                      v-focus
                      @blur="handleSaveTitle(save.id)"
                      @keyup.enter="handleSaveTitle(save.id)"
                      class="w-full rounded border border-[rgba(121,217,255,0.5)] bg-black/50 px-1.5
                        py-0.5 text-sm font-bold text-white outline-none"
                    />
                    <div
                      v-else
                      @dblclick="startEditTitle(save)"
                      class="max-w-full cursor-pointer truncate text-sm font-bold text-white
                        transition-colors duration-200 select-none hover:text-sky-300"
                      :title="$t('settings.save.list.editTitleTip')"
                    >
                      {{ save.title || $t("settings.save.list.untitled") }}
                    </div>
                  </div>

                  <!-- Separator -->
                  <div class="my-2 w-full border-b border-dashed border-white/15"></div>

                  <!-- Line 3: Last Message -->
                  <div
                    class="line-clamp-2 h-[33px] text-xs leading-[1.4] text-white/65 italic"
                    :title="save.last_message"
                  >
                    {{ save.last_message || $t("settings.save.list.noMessage") }}
                  </div>
                </div>
              </div>

              <!-- Bottom: Buttons -->
              <div class="mt-4 flex gap-2 border-t border-white/5 pt-3">
                <button
                  @click="handleLoadSave(save.id)"
                  class="flex-1 cursor-pointer rounded-md border border-blue-500/40 bg-blue-500/25
                    px-3 py-1.5 text-xs font-semibold whitespace-nowrap text-white transition-all
                    duration-200 hover:bg-blue-500/45 hover:shadow-[0_0_10px_rgba(59,130,246,0.2)]
                    disabled:cursor-not-allowed disabled:opacity-50"
                  :disabled="actionLoading !== null"
                >
                  {{
                    actionLoading === save.id
                      ? $t("settings.save.action.reading")
                      : $t("settings.save.action.load")
                  }}
                </button>
                <button
                  @click="handleSaveGame(save.id)"
                  class="flex-1 cursor-pointer rounded-md border border-emerald-500/40
                    bg-emerald-500/25 px-3 py-1.5 text-xs font-semibold whitespace-nowrap text-white
                    transition-all duration-200 hover:bg-emerald-500/45
                    hover:shadow-[0_0_10px_rgba(16,185,129,0.2)] disabled:cursor-not-allowed
                    disabled:opacity-50"
                  :disabled="actionLoading !== null"
                >
                  {{
                    actionLoading === save.id
                      ? $t("settings.save.action.saving")
                      : $t("settings.save.action.overwrite")
                  }}
                </button>
                <button
                  @click="handleDeleteSave(save.id)"
                  class="flex-1 cursor-pointer rounded-md border border-red-500/40 bg-red-500/25
                    px-3 py-1.5 text-xs font-semibold whitespace-nowrap text-white transition-all
                    duration-200 hover:bg-red-500/45 hover:shadow-[0_0_10px_rgba(239,68,68,0.2)]
                    disabled:cursor-not-allowed disabled:opacity-50"
                  :disabled="actionLoading !== null"
                >
                  {{
                    actionLoading === save.id
                      ? $t("settings.save.action.deleting")
                      : $t("settings.save.action.delete")
                  }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
  import { ref, onMounted } from "vue";
  import { useI18n } from "vue-i18n";
  import { MenuPage, MenuItem } from "../../ui";
  import { Input } from "../../base";
  import { useGameStore } from "../../../stores/modules/game";
  import { applyWebInitData } from "../../../stores/modules/game/actions";
  import { eventQueue } from "../../../core/events/event-queue";
  import { useUIStore } from "../../../stores/modules/ui/ui";
  import { useDialogStore } from "../../../stores/modules/ui/dialog";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import type { SaveInfo } from "../../../types";
  import type { WebInitData } from "../../../api/services/game-info";
  import { Save as SaveIcon, PencilLine, LayoutList, Clock } from "lucide-vue-next";

  interface SaveListResponse {
    saves: SaveInfo[];
    total: number;
  }

  interface CreateSaveResponse {
    save_id: number;
    message: string;
  }

  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const dialogStore = useDialogStore();
  const { t } = useI18n();

  const saves = ref<SaveInfo[]>([]);
  const newSaveTitle = ref("");
  const loading = ref(false);
  const error = ref<string | null>(null);
  const actionLoading = ref<number | null>(null);

  // Title editing state
  const editingSaveId = ref<number | null>(null);
  const editTitleText = ref("");

  // Custom directive for input auto-focus
  const vFocus = {
    mounted: (el: HTMLInputElement) => el.focus(),
  };

  const startEditTitle = (save: SaveInfo) => {
    editingSaveId.value = save.id;
    editTitleText.value = save.title;
  };

  const handleSaveTitle = async (saveId: number) => {
    const newTitle = editTitleText.value.trim();
    if (!newTitle) {
      uiStore.showWarning({
        title: t("settings.save.msg.warnTitle"),
        message: t("settings.save.msg.nameRequired"),
      });
      editingSaveId.value = null;
      return;
    }

    const save = saves.value.find((s) => s.id === saveId);
    if (save && save.title === newTitle) {
      editingSaveId.value = null;
      return;
    }

    try {
      await invoke("update_save_title", { saveId, title: newTitle });
      if (save) {
        save.title = newTitle;
      }
      uiStore.showSuccess({
        title: t("settings.save.msg.renameSuccessTitle"),
        message: t("settings.save.msg.renameSuccessMsg"),
      });
    } catch (e: any) {
      console.error("修改存档名称失败:", e);
      uiStore.showError({
        title: t("settings.save.msg.renameFailTitle"),
        message: typeof e === "string" ? e : e.message || t("settings.save.msg.unknownError"),
      });
    } finally {
      editingSaveId.value = null;
    }
  };

  const formatDate = (dateString: string): string => {
    const date = new Date(dateString);
    const pad = (n: number) => n.toString().padStart(2, "0");
    return `${date.getFullYear()}.${pad(date.getMonth() + 1)}.${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
  };

  const fetchSaves = async () => {
    loading.value = true;
    error.value = null;
    try {
      const result = await invoke<SaveListResponse>("list_saves", {
        page: 1,
        pageSize: 50,
      });
      saves.value = result.saves;
    } catch (e: any) {
      console.error("获取存档列表失败:", e);
      error.value = typeof e === "string" ? e : e.message || t("settings.save.msg.unknownError");
    } finally {
      loading.value = false;
    }
  };

  /** 确保截图已就绪：若最新截图为空但仍有进行中的截图任务，等待它完成。 */
  const ensureScreenshot = async (): Promise<string | null> => {
    if (gameStore.latestScreenshot) return gameStore.latestScreenshot;
    if (gameStore.screenshotPending) {
      await gameStore.screenshotPending;
    }
    return gameStore.latestScreenshot;
  };

  const handleCreateSave = async () => {
    if (!newSaveTitle.value.trim()) {
      uiStore.showWarning({
        title: t("settings.save.msg.warnTitle"),
        message: t("settings.save.msg.nameEmpty"),
      });
      return;
    }
    actionLoading.value = -1;
    try {
      await invoke<CreateSaveResponse>("create_save", {
        title: newSaveTitle.value.trim(),
        screenshotPath: await ensureScreenshot(),
      });
      newSaveTitle.value = "";
      uiStore.showSuccess({
        title: t("settings.save.msg.createSuccessTitle"),
        message: t("settings.save.msg.createSuccessMsg"),
      });
      await fetchSaves();
    } catch (e: any) {
      console.error("创建存档失败:", e);
      uiStore.showError({
        title: t("settings.save.msg.createFailTitle"),
        message: typeof e === "string" ? e : e.message || t("settings.save.msg.unknownError"),
      });
    } finally {
      actionLoading.value = null;
    }
  };

  const handleLoadSave = async (saveId: number) => {
    const confirmed = await dialogStore.confirm(t("settings.save.msg.loadConfirm"));
    if (!confirmed) return;
    actionLoading.value = saveId;
    try {
      const gameInfo = await invoke<WebInitData>("load_save", { saveId });
      applyWebInitData(gameStore.$state, gameInfo);
      // 读档后丢弃旧会话残留事件队列（防止旧角色未说完的回复串进新存档对话，issue #796）
      eventQueue.clear();
      eventQueue.resume();
      uiStore.showSuccess({
        title: t("settings.save.msg.loadSuccessTitle"),
        message: t("settings.save.msg.loadSuccessMsg"),
      });
    } catch (e: any) {
      console.error("读取存档失败:", e);
      uiStore.showError({
        title: t("settings.save.msg.loadFailTitle"),
        message: typeof e === "string" ? e : e.message || t("settings.save.msg.unknownError"),
      });
    } finally {
      actionLoading.value = null;
    }
  };

  const handleSaveGame = async (saveId: number) => {
    const confirmed = await dialogStore.confirm(t("settings.save.msg.overwriteConfirm"));
    if (!confirmed) return;
    actionLoading.value = saveId;
    try {
      await invoke("update_save", {
        saveId,
        screenshotPath: await ensureScreenshot(),
      });
      uiStore.showSuccess({
        title: t("settings.save.msg.overwriteSuccessTitle"),
        message: t("settings.save.msg.overwriteSuccessMsg"),
      });
      await fetchSaves();
    } catch (e: any) {
      console.error("保存游戏失败:", e);
      uiStore.showError({
        title: t("settings.save.msg.overwriteFailTitle"),
        message: typeof e === "string" ? e : e.message || t("settings.save.msg.unknownError"),
      });
    } finally {
      actionLoading.value = null;
    }
  };

  const handleDeleteSave = async (saveId: number) => {
    if (!(await dialogStore.confirm(t("settings.save.msg.deleteConfirm")))) return;
    actionLoading.value = saveId;
    try {
      await invoke("delete_save", { saveId });
      uiStore.showSuccess({
        title: t("settings.save.msg.deleteSuccessTitle"),
        message: t("settings.save.msg.deleteSuccessMsg"),
      });
      await fetchSaves();
    } catch (e: any) {
      console.error("删除存档失败:", e);
      uiStore.showError({
        title: t("settings.save.msg.deleteFailTitle"),
        message: typeof e === "string" ? e : e.message || t("settings.save.msg.unknownError"),
      });
    } finally {
      actionLoading.value = null;
    }
  };

  onMounted(() => {
    fetchSaves();
  });
</script>

<style scoped>
  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
</style>

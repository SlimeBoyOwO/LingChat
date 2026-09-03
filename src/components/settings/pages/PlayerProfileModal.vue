<template>
  <Transition name="modal">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm"
      @click="handleClose"
    >
      <div
        class="flex h-[85dvh] w-full max-w-4xl flex-col overflow-hidden rounded-3xl border
          border-white/20
          bg-[linear-gradient(135deg,rgba(255,255,255,0.15)_0%,rgba(255,255,255,0.05)_100%)]
          text-white shadow-[0_20px_60px_rgba(0,0,0,0.4),inset_0_0_1px_rgba(255,255,255,0.3)]
          backdrop-blur-[30px] backdrop-saturate-180"
        @click.stop
      >
        <!-- Header -->
        <div
          class="flex items-center justify-between border-b border-white/10
            bg-[linear-gradient(180deg,rgba(255,255,255,0.1)_0%,rgba(255,255,255,0.05)_100%)] p-6"
        >
          <div class="flex items-center gap-4">
            <div
              class="flex h-12 w-12 items-center justify-center rounded-xl bg-white/10 shadow-inner"
            >
              <Icon icon="setting" />
            </div>
            <div>
              <h2 class="m-0 text-xl font-bold drop-shadow-[0_2px_4px_rgba(0,0,0,0.3)]">
                {{ $t("settings.playerProfile.modalTitle") }}
              </h2>
              <p class="m-0 text-sm text-white/50">
                {{ $t("settings.playerProfile.modalSubtitle") }}
              </p>
            </div>
          </div>
          <button
            class="flex h-9 w-9 cursor-pointer items-center justify-center rounded-full border-none
              bg-white/10 text-white transition-all duration-200 hover:rotate-90 hover:bg-white/20"
            @click="handleClose"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="24"
              height="24"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>

        <!-- Content -->
        <div class="flex flex-1 flex-row overflow-hidden">
          <!-- Sidebar -->
          <div
            class="tab-sidebar-scroll flex w-44 shrink-0 flex-col gap-2 overflow-y-auto border-r
              border-white/10 bg-black/10 p-3"
          >
            <!-- 人设切换（多卡并存）：选中即加载该卡内容，保存写入所选卡 -->
            <div class="mb-2 border-b border-white/10 pb-3">
              <label class="mb-1 block text-[11px] font-medium text-white/50">{{
                $t("settings.playerProfile.activeProfile")
              }}</label>
              <select
                v-model="selectedProfileId"
                class="form-control w-full rounded-lg border border-white/10 bg-black/20 px-2 py-1.5
                  text-xs text-white outline-none"
                @change="onSelectPersona"
              >
                <option v-for="p in userStore.playerProfiles" :key="p.card_id" :value="p.card_id">
                  {{ p.user_name }}{{ p.active ? " ★" : "" }}
                </option>
                <option v-if="userStore.playerProfiles.length === 0" value="default">
                  default
                </option>
              </select>
              <div class="mt-2 flex gap-1.5">
                <button
                  class="flex-1 cursor-pointer rounded-md border-none bg-white/10 px-1.5 py-1
                    text-[11px] text-white transition-all duration-200 hover:bg-white/20
                    disabled:cursor-not-allowed disabled:opacity-40"
                  :disabled="selectedProfileId === userStore.activeProfileId"
                  @click="onSwitchProfile"
                >
                  {{ $t("settings.playerProfile.switchProfile") }}
                </button>
                <button
                  class="flex-1 cursor-pointer rounded-md border-none bg-white/10 px-1.5 py-1
                    text-[11px] text-white transition-all duration-200 hover:bg-white/20"
                  @click="newProfileVisible = !newProfileVisible"
                >
                  +
                </button>
                <button
                  class="flex-1 cursor-pointer rounded-md border-none bg-white/10 px-1.5 py-1
                    text-[11px] text-rose-300 transition-all duration-200 hover:bg-white/20"
                  @click="onDeleteProfile"
                >
                  −
                </button>
              </div>
              <!-- 新建人设内联表单：只填昵称，卡 ID 自动生成 -->
              <div v-if="newProfileVisible" class="mt-2 space-y-1.5">
                <input
                  v-model="newProfileName"
                  :placeholder="$t('settings.playerProfile.profileNamePlaceholder')"
                  class="form-control w-full rounded-md border border-white/10 bg-black/20 px-2 py-1
                    text-[11px] text-white outline-none"
                  @keydown.enter="onCreateProfile"
                />
                <p class="m-0 text-[10px] leading-snug text-white/40">
                  {{ $t("settings.playerProfile.profileIdHint") }}
                </p>
                <button
                  class="w-full cursor-pointer rounded-md border-none bg-[#5e72e4] px-2 py-1
                    text-[11px] font-medium text-white transition-all duration-200
                    hover:bg-[#4a5acf] disabled:cursor-not-allowed disabled:opacity-60"
                  :disabled="creatingProfile"
                  @click="onCreateProfile"
                >
                  {{ creatingProfile ? "..." : $t("settings.playerProfile.createProfile") }}
                </button>
              </div>
            </div>

            <button
              v-for="tab in tabs"
              :key="tab.id"
              class="w-full cursor-pointer rounded-xl border-none bg-transparent px-4 py-2.5
                text-left font-medium text-white/60 transition-all duration-200 hover:bg-white/5
                hover:text-white"
              :class="{
                'bg-[rgba(94,114,228,0.2)] font-semibold! text-[#79d9ff]!': activeTab === tab.id,
              }"
              @click="activeTab = tab.id"
            >
              {{ tab.label }}
            </button>
          </div>

          <!-- Tab Panels -->
          <div class="relative flex-1 overflow-y-auto p-6">
            <!-- 基础 tab：玩家名 / 副标题 / 简介 -->
            <div v-if="activeTab === 'basic'" class="mx-auto max-w-3xl space-y-4">
              <div class="flex flex-col gap-2">
                <label class="text-[13px] font-medium text-white/60">{{
                  $t("settings.playerProfile.userName")
                }}</label>
                <input
                  v-model="form.user_name"
                  type="text"
                  :placeholder="$t('settings.playerProfile.userNamePlaceholder')"
                  class="form-control rounded-xl border border-white/10 bg-black/20 px-3.5 py-2.5
                    text-sm text-white transition-all duration-200 outline-none"
                />
              </div>
              <div class="flex flex-col gap-2">
                <label class="text-[13px] font-medium text-white/60">{{
                  $t("settings.playerProfile.userSubtitle")
                }}</label>
                <input
                  v-model="form.user_subtitle"
                  type="text"
                  :placeholder="$t('settings.playerProfile.userSubtitlePlaceholder')"
                  class="form-control rounded-xl border border-white/10 bg-black/20 px-3.5 py-2.5
                    text-sm text-white transition-all duration-200 outline-none"
                />
              </div>
              <div class="flex flex-col gap-2">
                <label class="text-[13px] font-medium text-white/60">{{
                  $t("settings.playerProfile.playerInfo")
                }}</label>
                <textarea
                  v-model="form.info"
                  rows="4"
                  :placeholder="$t('settings.playerProfile.playerInfoPlaceholder')"
                  class="form-control resize-none rounded-xl border border-white/10 bg-black/20
                    px-3.5 py-2.5 text-sm leading-relaxed text-white transition-all duration-200
                    outline-none"
                ></textarea>
              </div>
            </div>

            <!-- 设定 tab：人格设定 / 说话示例 -->
            <div v-else-if="activeTab === 'prompts'" class="mx-auto max-w-3xl space-y-4">
              <div class="flex flex-col gap-2">
                <label class="text-[13px] font-medium text-white/60">{{
                  $t("settings.playerProfile.userPrompt")
                }}</label>
                <textarea
                  v-model="form.user_prompt"
                  rows="10"
                  :placeholder="$t('settings.playerProfile.userPromptPlaceholder')"
                  class="form-control resize-none rounded-xl border border-white/10 bg-black/20
                    px-3.5 py-2.5 font-mono text-sm leading-relaxed text-white transition-all
                    duration-200 outline-none"
                ></textarea>
                <p class="text-[0.68rem] leading-[1.6] text-white/40">
                  {{ $t("settings.playerProfile.userPromptHint") }}
                </p>
              </div>
              <div class="flex flex-col gap-2">
                <label class="text-[13px] font-medium text-white/60">{{
                  $t("settings.playerProfile.promptExample")
                }}</label>
                <textarea
                  v-model="form.system_prompt_example"
                  rows="6"
                  :placeholder="$t('settings.playerProfile.promptExamplePlaceholder')"
                  class="form-control resize-none rounded-xl border border-white/10 bg-black/20
                    px-3.5 py-2.5 font-mono text-sm leading-relaxed text-white transition-all
                    duration-200 outline-none"
                ></textarea>
              </div>
            </div>
          </div>
        </div>

        <!-- Footer -->
        <div
          class="flex items-center justify-between gap-3 border-t border-white/10
            bg-[linear-gradient(180deg,rgba(255,255,255,0.05)_0%,rgba(255,255,255,0.1)_100%)] p-4"
        >
          <!-- 保存失败的内联错误提示：不关闭弹窗，便于用户原地重试 -->
          <p v-if="inlineError" class="m-0 min-w-0 flex-1 text-sm leading-relaxed text-rose-300">
            {{ inlineError }}
          </p>
          <div class="flex shrink-0 gap-3">
            <button
              class="cursor-pointer rounded-[20px] border-none bg-white/10 px-5 py-2 text-sm
                font-medium text-white transition-all duration-200 hover:bg-white/20"
              @click="handleClose"
            >
              {{ $t("settings.playerProfile.cancel") }}
            </button>
            <button
              class="cursor-pointer rounded-[20px] border-none bg-[#5e72e4] px-5 py-2 text-sm
                font-medium text-white transition-all duration-200 hover:enabled:-translate-y-px
                hover:enabled:bg-[#4a5acf] hover:enabled:shadow-[0_4px_12px_rgba(94,114,228,0.3)]
                disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="saving"
              @click="saveSettings"
            >
              <span
                v-if="saving"
                class="mr-2 inline-block h-3.5 w-3.5 animate-spin rounded-full border-2
                  border-white/30 border-t-white"
              ></span>
              {{ saving ? $t("settings.playerProfile.saving") : $t("settings.playerProfile.save") }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
  import { computed, ref, watch } from "vue";
  import { useI18n } from "vue-i18n";
  import { Icon } from "../../base";
  import { useUserStore } from "../../../stores/modules/user/user";
  import { useUIStore } from "../../../stores/modules/ui/ui";
  import type { PlayerProfile } from "../../../api/services/game-info";

  const props = defineProps<{
    visible: boolean;
    profile: PlayerProfile;
  }>();

  const emit = defineEmits<{
    (e: "update:visible", value: boolean): void;
    (e: "saved"): void;
  }>();

  const { t } = useI18n();
  const userStore = useUserStore();
  const uiStore = useUIStore();

  const activeTab = ref("basic");
  const saving = ref(false);
  const inlineError = ref("");

  // 人设切换（多卡）状态：下拉选中即加载内容，编辑与保存都作用于「所选人设卡」
  const selectedProfileId = ref("default");
  const newProfileVisible = ref(false);
  const newProfileName = ref("");
  const creatingProfile = ref(false);

  // 本地表单副本（头像功能已移除；内容属于 selectedProfileId 对应的人设卡）
  const form = ref<PlayerProfile>({
    user_name: "玩家",
    user_subtitle: "",
    user_prompt: "",
    info: "",
    system_prompt_example: "",
  });

  const tabs = computed(() => [
    { id: "basic", label: t("settings.playerProfile.tabs.basic") },
    { id: "prompts", label: t("settings.playerProfile.tabs.prompts") },
  ]);

  // 打开弹窗：刷新人设列表后定位当前激活人设并加载其内容
  watch(
    () => props.visible,
    async (visible) => {
      if (!visible) return;
      inlineError.value = "";
      newProfileVisible.value = false;
      newProfileName.value = "";
      try {
        await userStore.loadPlayerProfiles();
        const target = resolveDefaultPersonaId();
        selectedProfileId.value = target;
        await refreshFormForSelected(target);
      } catch (e) {
        // 列表加载失败不阻断弹窗（表单仍可编辑默认档案）
        console.error("加载人设列表失败:", e);
        form.value = { ...props.profile };
      }
    }
  );

  /** 默认定位：当前激活人设；列表中不存在时回退到第一张卡 / "default" */
  function resolveDefaultPersonaId(): string {
    const list = userStore.playerProfiles;
    if (list.some((p) => p.card_id === userStore.activeProfileId)) {
      return userStore.activeProfileId;
    }
    return list[0]?.card_id || userStore.activeProfileId || "default";
  }

  /** 把所选人设卡的内容加载进表单（激活卡走全局档案，非激活卡按卡读取） */
  async function refreshFormForSelected(cardId: string) {
    if (cardId === userStore.activeProfileId) {
      await userStore.loadPlayerProfile();
      form.value = { ...userStore.playerProfile };
    } else {
      form.value = await userStore.loadPersonaProfile(cardId);
    }
  }

  const handleClose = () => {
    if (saving.value) return;
    emit("update:visible", false);
  };

  /** 下拉选中某张卡：只加载其内容供编辑/查看，不改变激活位 */
  async function onSelectPersona() {
    const id = selectedProfileId.value;
    if (!id) return;
    try {
      await refreshFormForSelected(id);
    } catch (e) {
      inlineError.value = errorMessage(e);
    }
  }

  /** 把所选人设设为当前激活（选中 ≠ 激活，需显式切换） */
  async function onSwitchProfile() {
    const target = selectedProfileId.value;
    if (!target || target === userStore.activeProfileId) return;
    try {
      await userStore.switchProfile(target);
      form.value = { ...userStore.playerProfile };
      uiStore.showSuccess({ message: t("settings.playerProfile.saved") });
    } catch (e) {
      inlineError.value = errorMessage(e);
      uiStore.showError({
        title: t("stores.notification.errorTitle"),
        message: errorMessage(e),
      });
    }
  }

  /**
   * 新建人设：只让用户填一次昵称，卡 ID 由昵称自动生成
   * （后端已改为「创建即激活」，这里随后同步列表与激活档案）。
   */
  async function onCreateProfile() {
    if (creatingProfile.value) return; // 防双击重复建卡
    const name = newProfileName.value.trim();
    if (!name) {
      inlineError.value = t("settings.playerProfile.profileNameRequired");
      return;
    }
    const cardId = uniquePersonaId(
      name,
      userStore.playerProfiles.map((p) => p.card_id)
    );
    creatingProfile.value = true;
    try {
      await userStore.createProfile(cardId, {
        user_name: name,
        user_subtitle: "",
        user_prompt: "",
        info: "",
        system_prompt_example: "",
      });
      // 后端创建即激活：刷新激活档案与人设列表后，让表单定位到新卡
      selectedProfileId.value = cardId;
      await refreshFormForSelected(cardId);
      newProfileVisible.value = false;
      newProfileName.value = "";
      uiStore.showSuccess({ message: t("settings.playerProfile.saved") });
    } catch (e) {
      inlineError.value = errorMessage(e);
      uiStore.showError({
        title: t("stores.notification.errorTitle"),
        message: errorMessage(e),
      });
    } finally {
      creatingProfile.value = false;
    }
  }

  /** 删除所选人设（禁止删除当前激活人设） */
  async function onDeleteProfile() {
    const target = selectedProfileId.value;
    if (!target) return;
    if (target === userStore.activeProfileId) {
      inlineError.value = t("settings.playerProfile.cannotDeleteActive");
      uiStore.showError({
        title: t("stores.notification.errorTitle"),
        message: t("settings.playerProfile.cannotDeleteActive"),
      });
      return;
    }
    if (!window.confirm(t("settings.playerProfile.confirmDelete"))) return;
    try {
      await userStore.deleteProfile(target);
      // 删除的是当前编辑卡：回到激活人设继续编辑
      const next = resolveDefaultPersonaId();
      selectedProfileId.value = next;
      await refreshFormForSelected(next);
      uiStore.showSuccess({ message: t("settings.playerProfile.saved") });
    } catch (e) {
      inlineError.value = errorMessage(e);
      uiStore.showError({
        title: t("stores.notification.errorTitle"),
        message: errorMessage(e),
      });
    }
  }

  /** 保存：写入「当前选中的人设卡」；选中即激活卡时走全局保存（带运行时热更新） */
  async function saveSettings() {
    saving.value = true;
    inlineError.value = "";
    const targetId = selectedProfileId.value || userStore.activeProfileId || "default";
    const fields: PlayerProfile = {
      user_name: form.value.user_name.trim() || "玩家",
      user_subtitle: form.value.user_subtitle.trim(),
      user_prompt: form.value.user_prompt,
      info: form.value.info,
      system_prompt_example: form.value.system_prompt_example,
    };
    try {
      if (targetId === userStore.activeProfileId) {
        await userStore.savePlayerProfile(fields);
        form.value = { ...userStore.playerProfile };
      } else {
        await userStore.savePersonaProfile(targetId, fields);
      }
      uiStore.showSuccess({ message: t("settings.playerProfile.saved") });
      emit("saved");
      emit("update:visible", false);
    } catch (e) {
      const message = errorMessage(e);
      inlineError.value = message;
      uiStore.showError({
        title: t("stores.notification.errorTitle"),
        message,
      });
      console.error("保存玩家档案失败:", e);
    } finally {
      saving.value = false;
    }
  }

  /** 将 Tauri invoke 的字符串错误 / JS Error 统一转成可展示文本 */
  function errorMessage(error: unknown): string {
    if (typeof error === "string" && error.trim()) return error;
    if (error instanceof Error && error.message) return error.message;
    return t("stores.notification.unknownError");
  }

  /**
   * 把昵称转成人设卡 id（沿用原目录名语义：可读、稳定、可用于剧本 persona_id）。
   * 保留字母/数字/下划线/连字符与中日韩文字，其余字符折叠为连字符。
   */
  function slugifyPersonaId(name: string): string {
    const slug = name
      .trim()
      .toLowerCase()
      .replace(/[^\p{L}\p{N}_-]+/gu, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 60)
      .replace(/-+$/g, "");
    return slug || "persona";
  }

  /** 生成不与现有列表冲突的 id：slug、slug-2、slug-3…… */
  function uniquePersonaId(name: string, existing: string[]): string {
    const base = slugifyPersonaId(name);
    const used = new Set(existing);
    if (!used.has(base)) return base;
    for (let n = 2; n <= 999; n++) {
      const candidate = `${base}-${n}`;
      if (!used.has(candidate)) return candidate;
    }
    return `${base}-${Date.now().toString(36)}`;
  }
</script>

<style scoped>
  /* 竖向侧边栏：细滚动条 */
  .tab-sidebar-scroll {
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.18) transparent;
  }
  .tab-sidebar-scroll::-webkit-scrollbar {
    width: 6px;
  }
  .tab-sidebar-scroll::-webkit-scrollbar-track {
    background: transparent;
  }
  .tab-sidebar-scroll::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.18);
    border-radius: 3px;
  }
  .form-control:focus {
    border-color: #79d9ff;
    background: rgba(0, 0, 0, 0.3);
    box-shadow: 0 0 0 3px rgba(121, 217, 255, 0.2);
  }

  .modal-enter-active,
  .modal-leave-active {
    transition: all 0.25s ease;
  }
  .modal-enter-from,
  .modal-leave-to {
    opacity: 0;
    transform: translateY(8px);
  }
</style>

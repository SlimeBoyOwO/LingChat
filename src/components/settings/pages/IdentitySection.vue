<template>
  <!-- 身份管理区块：内嵌进「角色」Tab 顶部，不再独占一个设置 Tab -->
  <MenuItem :title="$t('settings.identity.listTitle')">
    <template #header>
      <Users :size="20" />
    </template>

    <!-- 顶部操作：新建身份 -->
    <div class="mb-3 flex items-center justify-between gap-3">
      <p class="m-0 text-xs text-white/50">{{ $t("settings.identity.listHint") }}</p>
      <button
        type="button"
        class="flex shrink-0 cursor-pointer items-center gap-1.5 rounded-lg border-none bg-[var(--accent-color)] px-3 py-1.5 text-xs font-medium text-white transition-all duration-200 hover:-translate-y-0.5 hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)]"
        @click="startCreate"
      >
        <UserPlus :size="14" />
        {{ $t("settings.identity.create") }}
      </button>
    </div>

    <!-- 加载态 -->
    <div v-if="loading" class="flex items-center justify-center py-8">
      <div
        class="h-8 w-8 animate-spin rounded-full border-2 border-white/10 border-t-[#79d9ff]"
      ></div>
    </div>

    <!-- 空态：默认身份恒存在，正常不会出现 -->
    <p v-else-if="identities.length === 0" class="py-8 text-center text-sm text-white/40">
      {{ $t("settings.identity.empty") }}
    </p>

    <!-- 身份卡片列表 -->
    <div v-else class="space-y-3">
      <div
        v-for="item in identities"
        :key="item.role_id"
        class="rounded-xl border border-white/10 bg-white/5 p-4 backdrop-blur-md transition-colors duration-200"
        :class="isPossessed(item) ? 'border-emerald-400/40 bg-emerald-300/10' : ''"
      >
        <div class="flex items-start justify-between gap-3">
          <div class="flex min-w-0 flex-1 items-start gap-2">
            <UserRound :size="18" class="mt-0.5 shrink-0 text-white/60" />
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-2">
                <h3 class="truncate text-base font-bold text-white">{{ item.name }}</h3>
                <!-- 默认身份（id=0）徽标，永存且不可删 -->
                <span
                  v-if="item.role_id === DEFAULT_IDENTITY_ID"
                  class="shrink-0 rounded-full border border-amber-300/40 bg-amber-300/10 px-2 py-0.5 text-[10px] text-amber-200"
                >
                  {{ $t("settings.identity.defaultBadge") }}
                </span>
                <span
                  v-if="isPossessed(item)"
                  class="shrink-0 rounded-full border border-emerald-400/40 bg-emerald-300/10 px-2 py-0.5 text-[10px] text-emerald-200"
                >
                  {{ $t("settings.identity.possessedBadge") }}
                </span>
              </div>
              <p class="mt-0.5 truncate text-xs text-white/60">
                {{ item.profile.subtitle || $t("settings.identity.noSubtitle") }}
              </p>
              <p v-if="item.profile.info" class="mt-1 line-clamp-2 text-xs text-white/45">
                {{ item.profile.info }}
              </p>
            </div>
          </div>

          <div class="flex shrink-0 items-center gap-1.5">
            <!-- 扮演：切到该身份；被附身中或剧本/试玩进行中时禁用 -->
            <button
              type="button"
              class="flex items-center gap-1 rounded-md border px-2 py-1 text-[11px] transition-colors"
              :class="
                isPossessed(item) || possessBlockedReason(item)
                  ? 'cursor-not-allowed border-white/10 bg-white/5 text-white/25'
                  : `cursor-pointer border-emerald-400/30 bg-emerald-500/10 text-emerald-200 hover:bg-emerald-500/25`
              "
              :disabled="isPossessed(item) || !!possessBlockedReason(item) || possessingId !== null"
              :title="possessBlockedReason(item) || $t('settings.identity.possess')"
              @click="possess(item)"
            >
              <Loader2 v-if="possessingId === item.role_id" :size="12" class="animate-spin" />
              <Play v-else :size="12" />
              {{
                isPossessed(item)
                  ? $t("settings.identity.possessedBadge")
                  : $t("settings.identity.possess")
              }}
            </button>
            <button
              type="button"
              class="flex cursor-pointer items-center gap-1 rounded-md border border-white/10 bg-white/5 px-2 py-1 text-[11px] text-white/70 transition-colors hover:bg-white/10 hover:text-white"
              @click="startEdit(item)"
            >
              <Pencil :size="12" />
              {{ $t("settings.identity.edit") }}
            </button>
            <button
              type="button"
              class="flex items-center gap-1 rounded-md border px-2 py-1 text-[11px] transition-colors"
              :class="
                isDeletable(item)
                  ? `cursor-pointer border-red-400/30 bg-red-500/10 text-red-200 hover:bg-red-500/25`
                  : 'cursor-not-allowed border-white/10 bg-white/5 text-white/25'
              "
              :disabled="!isDeletable(item)"
              :title="
                isDeletable(item)
                  ? $t('settings.identity.delete')
                  : $t('settings.identity.defaultDeleteBlocked')
              "
              @click="confirmDelete(item)"
            >
              <Trash2 :size="12" />
              {{ $t("settings.identity.delete") }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </MenuItem>

  <!-- 新建 / 编辑表单 -->
  <MenuItem
    v-if="editorOpen"
    :title="
      editingRoleId === null
        ? $t('settings.identity.createTitle')
        : $t('settings.identity.editTitle')
    "
  >
    <template #header>
      <UserPen :size="20" />
    </template>

    <div class="space-y-4">
      <div class="flex flex-col gap-2">
        <label class="text-[13px] font-medium text-white/60" for="identity-name">
          {{ $t("settings.identity.fields.name") }} *
        </label>
        <input
          id="identity-name"
          v-model="form.name"
          type="text"
          :placeholder="$t('settings.identity.fields.namePlaceholder')"
          class="form-control rounded-xl border border-white/10 bg-black/20 px-3.5 py-2.5 text-sm text-white transition-all duration-200 outline-none"
        />
      </div>

      <div class="flex flex-col gap-2">
        <label class="text-[13px] font-medium text-white/60" for="identity-subtitle">
          {{ $t("settings.identity.fields.subtitle") }}
        </label>
        <input
          id="identity-subtitle"
          v-model="form.subtitle"
          type="text"
          :placeholder="$t('settings.identity.fields.subtitlePlaceholder')"
          class="form-control rounded-xl border border-white/10 bg-black/20 px-3.5 py-2.5 text-sm text-white transition-all duration-200 outline-none"
        />
      </div>

      <div class="flex flex-col gap-2">
        <label class="text-[13px] font-medium text-white/60" for="identity-info">
          {{ $t("settings.identity.fields.info") }}
        </label>
        <textarea
          id="identity-info"
          v-model="form.info"
          rows="5"
          :placeholder="$t('settings.identity.fields.infoPlaceholder')"
          class="form-control resize-y rounded-xl border border-white/10 bg-black/20 px-3.5 py-2.5 text-sm leading-relaxed text-white transition-all duration-200 outline-none"
        ></textarea>
      </div>

      <div class="flex justify-end gap-3 pt-1">
        <button
          type="button"
          class="cursor-pointer rounded-[20px] border-none bg-white/10 px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:bg-white/20"
          @click="closeEditor"
        >
          {{ $t("settings.identity.actions.cancel") }}
        </button>
        <button
          type="button"
          class="cursor-pointer rounded-[20px] border-none bg-[#5e72e4] px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:enabled:-translate-y-px hover:enabled:bg-[#4a5acf] disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="saving || !form.name.trim()"
          @click="submitForm"
        >
          <span
            v-if="saving"
            class="mr-2 inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/30 border-t-white"
          ></span>
          {{
            saving ? $t("settings.identity.actions.saving") : $t("settings.identity.actions.save")
          }}
        </button>
      </div>
    </div>
  </MenuItem>
</template>

<script setup lang="ts">
import { computed, onActivated, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  Loader2,
  Pencil,
  Play,
  Trash2,
  UserPen,
  UserPlus,
  UserRound,
  Users,
} from "lucide-vue-next";
import { MenuItem } from "../../ui";
import {
  createIdentity,
  deleteIdentity,
  getPossessedEntity,
  listIdentities,
  possessEntity,
  updateIdentity,
  type IdentityInfo,
  type RoleProfile,
} from "../../../api/services/identity";
import { useGameStore } from "../../../stores/modules/game";
import { useDialogStore } from "../../../stores/modules/ui/dialog";
import { useUIStore } from "../../../stores/modules/ui/ui";

/** 默认身份哨兵 id：永存不可删，但可改名改人设 */
const DEFAULT_IDENTITY_ID = 0;

const gameStore = useGameStore();
const uiStore = useUIStore();
const dialogStore = useDialogStore();
const { t } = useI18n();

const identities = ref<IdentityInfo[]>([]);
const loading = ref(false);
const saving = ref(false);
/** 正在附身的身份 id（按钮 loading，避免重复点击） */
const possessingId = ref<number | null>(null);

const editorOpen = ref(false);
/** null = 新建模式；数字 = 正在编辑的实体 id */
const editingRoleId = ref<number | null>(null);
/** 编辑时缓存原 profile，保留 location_id 等本期不编辑的预留字段 */
const editingProfile = ref<RoleProfile | null>(null);

const form = reactive({
  name: "",
  subtitle: "",
  info: "",
});

/** 剧本/试玩进行中禁止附身（与后端 gs.script_status 校验同源） */
const scriptRunning = computed(() => gameStore.runningScript?.isRunning === true);

/** 当前被附身身份以 store 为准（后端广播即时同步），不再依赖列表接口的 possessed 快照 */
const isPossessed = (item: IdentityInfo) => gameStore.possessedRoleId === item.role_id;

/** 返回非空字符串表示禁用原因，空串表示可附身 */
const possessBlockedReason = (item: IdentityInfo): string => {
  if (scriptRunning.value) return t("settings.identity.possessDisabledInScript");
  if (isPossessed(item)) return t("settings.identity.possessedBadge");
  return "";
};

const resetForm = () => {
  form.name = "";
  form.subtitle = "";
  form.info = "";
};

const loadIdentities = async () => {
  loading.value = true;
  try {
    identities.value = await listIdentities();
  } catch (error) {
    console.error("[IdentitySection] 获取身份列表失败:", error);
    uiStore.showNotification({
      type: "error",
      title: t("settings.identity.messages.loadFailed"),
      message: String(error),
      skipTipsCheck: true,
    });
  } finally {
    loading.value = false;
  }
};

/** 从后端拉取当前被附身实体，回写 store 三字段（首帧/刷新后恢复会话态展示） */
const loadPossessed = async () => {
  try {
    const info = await getPossessedEntity();
    gameStore.possessedRoleId = info.role_id;
    if (info.name) gameStore.userName = info.name;
    gameStore.userSubtitle = info.subtitle ?? "";
  } catch (error) {
    console.warn("[IdentitySection] 读取当前扮演身份失败:", error);
  }
};

const possess = async (item: IdentityInfo) => {
  if (isPossessed(item) || possessBlockedReason(item) || possessingId.value !== null) return;
  possessingId.value = item.role_id;
  try {
    const name = await possessEntity(item.role_id);
    // 立即回写本地态，不必等 identity:possessed 广播到达
    gameStore.possessedRoleId = item.role_id;
    gameStore.userName = name;
    uiStore.showNotification({
      type: "success",
      title: t("game.dialog.possessSuccessTitle"),
      message: t("game.dialog.possessSuccess", { name }),
      duration: 2000,
      skipTipsCheck: true,
    });
  } catch (error) {
    uiStore.showNotification({
      type: "warning",
      title: t("game.dialog.possessFailedTitle"),
      message: String(error),
      skipTipsCheck: true,
    });
  } finally {
    possessingId.value = null;
  }
};

const startCreate = () => {
  editingRoleId.value = null;
  editingProfile.value = null;
  resetForm();
  editorOpen.value = true;
};

const startEdit = (item: IdentityInfo) => {
  editingRoleId.value = item.role_id;
  editingProfile.value = item.profile;
  form.name = item.name;
  form.subtitle = item.profile.subtitle;
  form.info = item.profile.info;
  editorOpen.value = true;
};

const closeEditor = () => {
  editorOpen.value = false;
  editingRoleId.value = null;
  editingProfile.value = null;
  resetForm();
};

/** 组装提交用 profile：编辑时保留预留字段，新建时用空默认值 */
const buildProfile = (): RoleProfile => {
  const base: RoleProfile = editingProfile.value ?? {
    subtitle: "",
    info: "",
    location_id: null,
    home_location_id: null,
    attributes: {},
  };
  return {
    ...base,
    subtitle: form.subtitle.trim(),
    info: form.info.trim(),
  };
};

const submitForm = async () => {
  const name = form.name.trim();
  if (!name || saving.value) return;

  saving.value = true;
  try {
    const profile = buildProfile();
    if (editingRoleId.value === null) {
      await createIdentity(name, profile);
      uiStore.showNotification({
        type: "success",
        title: t("settings.identity.messages.createSuccessTitle"),
        message: t("settings.identity.messages.createSuccess", { name }),
        skipTipsCheck: true,
      });
    } else {
      await updateIdentity(editingRoleId.value, name, profile);
      uiStore.showNotification({
        type: "success",
        title: t("settings.identity.messages.updateSuccessTitle"),
        message: t("settings.identity.messages.updateSuccess", { name }),
        skipTipsCheck: true,
      });
    }
    closeEditor();
    await loadIdentities();
  } catch (error) {
    console.error("[IdentitySection] 保存身份失败:", error);
    uiStore.showNotification({
      type: "error",
      title: t("settings.identity.messages.saveFailed"),
      message: String(error),
      skipTipsCheck: true,
    });
  } finally {
    saving.value = false;
  }
};

/** 默认身份受系统保护，禁止删除 */
const isDeletable = (item: IdentityInfo) => item.role_id !== DEFAULT_IDENTITY_ID;

const confirmDelete = async (item: IdentityInfo) => {
  if (!isDeletable(item)) return;

  const confirmed = await dialogStore.confirm(
    t("settings.identity.messages.deleteConfirm", { name: item.name }),
    t("settings.identity.messages.deleteConfirmTitle"),
  );
  if (!confirmed) return;

  try {
    await deleteIdentity(item.role_id);
    uiStore.showNotification({
      type: "success",
      title: t("settings.identity.messages.deleteSuccessTitle"),
      message: t("settings.identity.messages.deleteSuccess", { name: item.name }),
      skipTipsCheck: true,
    });
    // 被编辑的身份若刚被删除，关闭表单避免继续写已失效实体
    if (editingRoleId.value === item.role_id) closeEditor();
    await loadIdentities();
  } catch (error) {
    console.error("[IdentitySection] 删除身份失败:", error);
    uiStore.showNotification({
      type: "error",
      title: t("settings.identity.messages.deleteFailed"),
      message: String(error),
      skipTipsCheck: true,
    });
  }
};

onMounted(() => {
  void loadIdentities();
  void loadPossessed();
});

// 设置面板是 v-show + KeepAlive：切走再切回不重跑 setup，靠 activated 补拉一次列表
onActivated(() => {
  void loadIdentities();
});

// 身份 CRUD / 角色设置保存后后端广播 role:list-updated → 版本号自增 → 重拉
watch(
  () => gameStore.roleListVersion,
  () => {
    void loadIdentities();
  },
);
</script>

<template>
  <Transition name="modal">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm"
      @click="handleClose"
    >
      <div
        class="glass-effect flex h-[85dvh] w-full max-w-4xl flex-col overflow-hidden rounded-3xl text-white"
        @click.stop
      >
        <!-- Header -->
        <div class="flex items-center justify-between border-b border-white/10 bg-white/5 p-6">
          <div class="flex items-center gap-4">
            <div
              class="flex h-12 w-12 items-center justify-center rounded-xl bg-white/10 shadow-inner"
            >
              <Icon icon="setting" />
            </div>
            <div>
              <h2 class="m-0 text-xl font-bold drop-shadow-[0_2px_4px_rgba(0,0,0,0.3)]">
                {{ $t("settings.characterInfo.header.title", { title: headerTitle }) }}
              </h2>
              <p class="m-0 text-sm text-white/50">
                {{ $t("settings.characterInfo.header.subtitle") }}
              </p>
            </div>
          </div>
          <button
            class="flex h-9 w-9 cursor-pointer items-center justify-center rounded-full border-none bg-white/10 text-white transition-all duration-200 hover:rotate-90 hover:bg-white/20"
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
            class="tab-sidebar-scroll flex w-44 shrink-0 flex-col gap-2 overflow-y-auto border-r border-white/10 bg-black/10 p-3"
          >
            <button
              v-for="tab in tabs"
              :key="tab.id"
              class="w-full cursor-pointer rounded-xl border-none bg-transparent px-4 py-2.5 text-left font-medium text-white/60 transition-all duration-200 hover:bg-white/5 hover:text-white"
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
            <div v-if="loading" class="flex h-full items-center justify-center">
              <div
                class="h-10 w-10 animate-spin rounded-full border-3 border-white/10 border-t-[#5e72e4]"
              ></div>
            </div>

            <fieldset
              v-else
              :inert="saving || resourceBusy > 0"
              :disabled="saving || resourceBusy > 0"
              class="mx-auto max-w-3xl min-w-0 space-y-6 border-0 p-0"
            >
              <p
                class="rounded-xl border border-cyan-200/15 bg-cyan-400/5 p-3 text-xs leading-relaxed text-cyan-100/70"
              >
                {{ t("settings.characterInfo.draft.hint") }}
              </p>
              <WorldbookEditor
                v-if="editId"
                v-show="activeTab === 'worldbook'"
                ref="worldbookEditor"
                v-model="worldbook"
                :baseline="worldbookBaseline"
                :load-error="worldbookError"
              />
              <PetLayoutEditor
                v-if="activeTab === 'pet' && props.roleId"
                v-model="localSettings"
                :role-id="props.roleId"
                :clothes="draftClothes"
              />
              <RoleLayoutEditor
                v-if="activeTab === 'visuals' && props.roleId"
                v-model="localSettings"
                :role-id="props.roleId"
                :clothes="draftClothes"
              />
              <!-- Data-Driven Form -->
              <div v-if="currentTabConfig" class="space-y-4">
                <div
                  v-for="(field, index) in currentTabFields"
                  :key="index"
                  class="flex flex-col gap-2"
                >
                  <label :for="field.key" class="text-[13px] font-medium text-white/60">{{
                    field.label
                  }}</label>
                  <input
                    v-if="field.type === 'text' || field.type === 'number'"
                    :id="field.key"
                    v-model="fieldModel(field).value"
                    :type="field.type"
                    :step="field.step"
                    :min="field.min"
                    :max="field.max"
                    :placeholder="field.placeholder"
                    class="glass-input"
                  />
                  <DialogueExamplesEditor
                    v-else-if="field.type === 'examples'"
                    v-model="fieldModel(field).value"
                    :user-name="localSettings.user_name || gameStore.userName"
                    :character-name="localSettings.ai_name || ''"
                  />
                  <textarea
                    v-else-if="field.type === 'textarea'"
                    :id="field.key"
                    v-model="fieldModel(field).value"
                    :rows="field.rows || 4"
                    class="glass-input font-mono"
                  ></textarea>
                  <select
                    v-else-if="field.type === 'select'"
                    :id="field.key"
                    v-model="fieldModel(field).value"
                    class="glass-input"
                  >
                    <option
                      v-for="opt in resolveFieldOptions(field)"
                      :key="opt.value"
                      :value="opt.value"
                      class="bg-[#333] text-white"
                    >
                      {{ opt.label }}
                    </option>
                  </select>
                  <!-- 开关：轨道是第二个 label，滑块用 peer-checked 联动 -->
                  <div v-else-if="field.type === 'switch'" class="relative flex w-fit items-center">
                    <input
                      :id="field.key"
                      v-model="fieldModel(field).value"
                      type="checkbox"
                      class="peer sr-only"
                    />
                    <label
                      :for="field.key"
                      class="peer-checked:border-brand peer-checked:bg-brand block h-5 w-9 cursor-pointer rounded-full border border-white/20 bg-white/10 transition-colors"
                    ></label>
                    <div
                      class="pointer-events-none absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white transition-transform peer-checked:translate-x-4"
                    ></div>
                  </div>
                  <p v-if="field.hint" class="text-[12px] leading-relaxed text-white/40">
                    {{ field.hint }}
                  </p>
                </div>
              </div>

              <!-- 关系（AI 角色视角）：角色间关系写 relations.yml，角色对玩家身份的关系写数据库 -->
              <CharacterRelationsEditor
                v-if="activeTab === 'basic' && props.roleId"
                ref="relationsEditor"
                :role-id="props.roleId"
                :character-folder="String(localSettings.character_folder || '')"
              />

              <!-- GSV 六情绪参考语音：填全的分类按情绪切换参考音色，未填全的
                   分类仍回退到上面的 gsv_voice_filename / gsv_voice_text -->
              <div
                v-if="activeTab === 'voice' && localSettings.tts_type === 'gsv'"
                class="space-y-4"
              >
                <div
                  class="flex items-center justify-between rounded-xl border border-white/10 bg-white/5 p-4"
                >
                  <div>
                    <h3 class="text-sm font-bold text-white/70">
                      {{ $t("settings.characterInfo.gsvEmo.title") }}
                    </h3>
                    <p class="mt-1 text-xs text-white/40">
                      {{ $t("settings.characterInfo.gsvEmo.description") }}
                    </p>
                  </div>
                  <button
                    role="switch"
                    :aria-checked="gsvEmoEnabled"
                    class="cursor-pointer rounded-lg border-none px-3.5 py-1.5 text-xs transition-colors"
                    :class="
                      gsvEmoEnabled
                        ? 'bg-[#5e72e4] text-white hover:bg-[#4a5acf]'
                        : 'bg-white/10 text-white/60 hover:bg-white/20'
                    "
                    @click="toggleGsvEmo"
                  >
                    {{
                      gsvEmoEnabled
                        ? $t("settings.characterInfo.gsvEmo.enable")
                        : $t("settings.characterInfo.gsvEmo.disable")
                    }}
                  </button>
                </div>
                <div v-if="gsvEmoEnabled" class="space-y-3">
                  <div
                    v-for="cat in GSV_EMO_CATEGORIES"
                    :key="cat"
                    class="grid grid-cols-1 gap-3 rounded-xl border border-white/10 bg-white/5 p-4 md:grid-cols-2"
                  >
                    <div class="flex flex-col gap-2">
                      <label class="text-[13px] font-medium text-white/60">
                        {{ cat }} · {{ $t("settings.characterInfo.gsvEmo.voiceFile") }}
                      </label>
                      <input
                        v-model="gsvEmoField(cat, 'gsv_emo_voice_files').value"
                        type="text"
                        :placeholder="$t('settings.characterInfo.gsvEmo.voiceFilePlaceholder')"
                        class="glass-input"
                      />
                    </div>
                    <div class="flex flex-col gap-2">
                      <label class="text-[13px] font-medium text-white/60">
                        {{ cat }} · {{ $t("settings.characterInfo.gsvEmo.text") }}
                      </label>
                      <input
                        v-model="gsvEmoField(cat, 'gsv_emo_texts').value"
                        type="text"
                        :placeholder="$t('settings.characterInfo.gsvEmo.textPlaceholder')"
                        class="glass-input"
                      />
                    </div>
                  </div>
                </div>
              </div>

              <VoicePreview
                v-if="activeTab === 'voice' && props.roleId"
                :role-id="props.roleId"
                :model-value="localSettings"
              />

              <AvatarManager
                v-if="activeTab === 'avatars' && props.roleId"
                :role-id="props.roleId"
                :clothes="clothesList"
                :resource-clothes="draftClothes"
                :initial-costume="avatarCostume"
              />

              <Live2DSettings
                v-if="activeTab === 'live2d' && props.roleId"
                v-model="localSettings.live2d"
                :role-id="props.roleId"
                :character-folder="localSettings.character_folder || ''"
                :clothes="clothesList"
                :scale="Number(localSettings.scale) || 1"
                :offset-x="Number(localSettings.offset_x) || 0"
                :offset-y="Number(localSettings.offset_y) || 0"
              />

              <CostumeManager
                v-if="activeTab === 'clothes' && props.roleId"
                v-model="localSettings"
                :role-id="props.roleId"
                @changed="onCostumeChanged"
                @edit-avatars="openCostumeAvatars"
              />

              <!-- Touch Tab：这里只做入口与概览 -->
              <div v-if="activeTab === 'touch'" class="space-y-4">
                <p class="text-[13px] leading-relaxed text-white/50">
                  {{ $t("settings.characterInfo.touch.hint") }}
                </p>
                <button
                  class="cursor-pointer rounded-lg border-none bg-[#5e72e4] px-3 py-1.5 text-xs text-white transition-colors hover:bg-[#4a5acf]"
                  :disabled="clothesLoading"
                  @click="touchEditorVisible = true"
                >
                  {{ $t("settings.characterInfo.touch.open") }}
                </button>
              </div>
            </fieldset>
          </div>
        </div>

        <!-- Footer -->
        <div class="flex justify-between gap-3 border-t border-white/10 bg-white/5 p-4">
          <!-- 危险操作区：删除角色 -->
          <div class="flex items-center">
            <button
              :disabled="deleteState.disabled || loading || saving || resourceBusy > 0"
              :title="
                deleteState.disabled
                  ? deleteState.reason
                  : t('settings.characterInfo.delete.button')
              "
              :class="[
                'rounded-[20px] border px-4 py-2 text-sm font-medium transition-all duration-200',
                deleteState.disabled
                  ? 'cursor-not-allowed border-white/10 bg-white/5 text-white/25'
                  : `border-red-400/30 bg-red-500/15 text-red-200 hover:-translate-y-px hover:bg-red-500/30 hover:shadow-[0_4px_12px_rgba(239,68,68,0.3)]`,
              ]"
              @click="deleteState.disabled ? null : handleDelete()"
            >
              <span class="inline-flex items-center gap-1.5">
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <polyline points="3 6 5 6 21 6"></polyline>
                  <path
                    d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
                  ></path>
                  <line x1="10" y1="11" x2="10" y2="17"></line>
                  <line x1="14" y1="11" x2="14" y2="17"></line>
                </svg>
                {{ t("settings.characterInfo.delete.button") }}
              </span>
            </button>
          </div>

          <!-- 普通操作：取消/保存 -->
          <div class="flex gap-3">
            <button
              class="cursor-pointer rounded-[20px] border-none bg-white/10 px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:bg-white/20"
              @click="handleClose"
            >
              {{ $t("settings.characterInfo.footer.cancel") }}
            </button>
            <button
              class="cursor-pointer rounded-[20px] border-none bg-[#5e72e4] px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:enabled:-translate-y-px hover:enabled:bg-[#4a5acf] hover:enabled:shadow-[0_4px_12px_rgba(94,114,228,0.3)] disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="saving || loading || resourceBusy > 0 || !loaded || !editId"
              @click="saveSettings"
            >
              <span
                v-if="saving"
                class="mr-2 inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/30 border-t-white"
              ></span>
              {{
                saving
                  ? $t("settings.characterInfo.footer.saving")
                  : $t("settings.characterInfo.footer.save")
              }}
            </button>
          </div>
        </div>

        <!-- 触摸区域编辑器 -->
        <TouchRegionsEditor
          :user-name="gameStore.userName"
          v-if="touchEditorVisible"
          :body-part="localSettings.body_part"
          :clothes="draftClothes"
          @apply="onTouchRegionsApply"
          @close="touchEditorVisible = false"
        />
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, onUnmounted, provide, ref, toRaw, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  deleteCharacter as deleteCharacterApi,
  beginCharacterEdit,
  discardCharacterEdit,
  commitCharacterEdit,
  getLive2dFilePath,
  listCharacterCostumes,
} from "../../../api/services/character";
import { confirmSaveEffect } from "../../../composables/usePreSaveConfirm";
import { resolveLive2dVariant } from "@/types/live2d";
import { characterEditorKey } from "@/composables/useCharacterEditor";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Icon } from "../../base";
import ExpandableInput from "../../ui/ExpandableInput.vue";
import Live2DSettings from "../character/Live2DSettings.vue";
import AvatarManager from "../character/AvatarManager.vue";
import WorldbookEditor from "../character/WorldbookEditor.vue";
import { readCharacterWorldbook, type Worldbook } from "@/api/services/character";
import RoleLayoutEditor from "../character/RoleLayoutEditor.vue";
import CostumeManager from "../character/CostumeManager.vue";
import PetLayoutEditor from "../character/PetLayoutEditor.vue";
import VoicePreview from "../character/VoicePreview.vue";
import DialogueExamplesEditor from "../character/DialogueExamplesEditor.vue";
import CharacterRelationsEditor from "./character-info/CharacterRelationsEditor.vue";
import { useVoiceResources } from "./character-info/useVoiceResources";
import { buildSchemas, type FieldOption, type FieldSchema } from "./character-info/fields";
import TouchRegionsEditor from "../character/TouchRegionsEditor.vue";
import { isSystemProtectedRole } from "@/constants/character";
import { useDialogStore } from "../../../stores/modules/ui/dialog";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";

const props = defineProps<{
  visible: boolean;
  roleId: number | null;
  title?: string;
  /** 来源："game" 或提供该角色的插件 id（插件角色不可直接删除）。 */
  source?: string | null;
  /** 该角色实际存在的服装（标题 + 立绘 URL），由设置列表页透传 */
  clothes?: Array<{ title: string; avatar: string }>;
}>();

const emit = defineEmits(["close", "saved", "loaded"]);

const activeTab = ref("basic");
const avatarCostume = ref("default");
const loading = ref(false);
/** 表单是否已完成一次成功装载；未装载时禁止保存，避免把空表单写回角色卡 */
const loaded = ref(false);
const saving = ref(false);
const deleting = ref(false);
const dialogStore = useDialogStore();
const { t } = useI18n();
const uiStore = useUIStore();
const gameStore = useGameStore();
const localSettings = ref<any>({});
const touchEditorVisible = ref(false);
const editId = ref<string>();
const editRoleId = ref<number>();
const resourceBusy = ref(0);
const resourceRevision = ref(0);
const baseline = ref("");
const worldbook = ref<Worldbook | null>(null);
const worldbookBaseline = ref<Worldbook | null>(null);
const worldbookError = ref("");
const worldbookEditor = ref<InstanceType<typeof WorldbookEditor>>();
const worldbookChanged = computed(
  () => JSON.stringify(worldbook.value) !== JSON.stringify(worldbookBaseline.value),
);
const draftClothes = ref<Array<{ title: string; avatar: string }>>([]);
const clothesLoading = ref(false);
let clothesSequence = 0;
let loadSequence = 0;
provide(characterEditorKey, { editId, busy: resourceBusy, revision: resourceRevision });
const hasChanges = computed(
  () =>
    loaded.value &&
    (worldbookChanged.value ||
      resourceRevision.value > 0 ||
      JSON.stringify(localSettings.value) !== baseline.value),
);
/** 标题兜底：关系引导可能先于角色列表拿到 title，装载后用角色名补上 */
const headerTitle = computed(
  () =>
    props.title?.trim() ||
    String(localSettings.value.ai_name ?? "").trim() ||
    t("settings.characterInfo.header.untitled"),
);
async function refreshDraftClothes() {
  const id = editId.value;
  const roleId = editRoleId.value;
  if (!id || !roleId) return;
  const sequence = ++clothesSequence;
  clothesLoading.value = true;
  try {
    const costumes = await listCharacterCostumes(roleId, id);
    if (editId.value !== id || sequence !== clothesSequence) return;
    draftClothes.value = costumes.map((costume) => ({
      title: costume.name,
      avatar: costume.preview
        ? convertFileSrc(costume.preview) + "?v=" + resourceRevision.value
        : "",
    }));
  } finally {
    if (sequence === clothesSequence) clothesLoading.value = false;
  }
}
watch([resourceRevision, activeTab, editId], () => {
  if (
    editId.value &&
    ["visuals", "avatars", "clothes", "touch", "live2d", "pet"].includes(activeTab.value)
  ) {
    void refreshDraftClothes().catch(console.error);
  }
});

async function discardDraft() {
  ++clothesSequence;
  clothesLoading.value = false;
  const id = editId.value;
  const roleId = editRoleId.value;
  if (id && roleId) await discardCharacterEdit(roleId, id);
  if (editId.value === id) {
    editId.value = undefined;
    editRoleId.value = undefined;
  }
}
// 语音相关状态与操作收敛在 composable 里，这里只做接线
const {
  installedVoices,
  cloudVoices,
  refreshLocalVoices,
  refreshCloudVoices,
  ensureVoiceModels,
  migrateLegacyVoiceModelFields,
  GSV_EMO_CATEGORIES,
  gsvEmoEnabled,
  toggleGsvEmo,
  gsvEmoField,
} = useVoiceResources(localSettings);

// 删除按钮可用性：系统保护角色 / 在场角色不可删
const deleteState = computed(() => {
  if (!props.roleId)
    return { disabled: true, reason: t("settings.characterInfo.delete.systemProtected") };
  if (isSystemProtectedRole(props.roleId)) {
    return { disabled: true, reason: t("settings.characterInfo.delete.systemProtected") };
  }
  const onstage =
    gameStore.mainRoleId === props.roleId || gameStore.presentRoleIds.includes(props.roleId);
  if (onstage) {
    return { disabled: true, reason: t("settings.characterInfo.delete.onstage") };
  }
  return { disabled: false, reason: "" };
});

// 单次 confirm，一并删除 DB / 存档 / 记忆 / 物理文件
const handleDelete = async () => {
  if (!props.roleId || deleteState.value.disabled) return;

  // 插件角色不可直接删除：提示去插件设置里隐藏
  if (props.source && props.source !== "game") {
    await dialogStore.alert(t("settings.characterInfo.delete.pluginFromPlugin"));
    return;
  }

  const confirmed = await dialogStore.confirm(
    t("settings.characterInfo.delete.confirmMessage", {
      title: props.title ?? t("settings.characterInfo.delete.button"),
    }),
    t("settings.characterInfo.delete.confirmTitle"),
  );
  if (!confirmed) return;

  deleting.value = true;
  try {
    await deleteCharacterApi(props.roleId, true);

    // 删除成功
    uiStore.showSuccess({
      title: t("settings.characterInfo.delete.successTitle"),
      message: t("settings.characterInfo.delete.successMessage", {
        title: props.title ?? t("settings.characterInfo.delete.button"),
      }),
    });

    // 通知父组件刷新列表 + 关闭弹窗
    emit("saved");
    emit("close");
  } catch (error: any) {
    console.error("[SettingsCharacterInfo] 删除角色失败:", error);
    uiStore.showError({
      title: t("settings.characterInfo.delete.failTitle"),
      message: typeof error === "string" ? error : error?.message || "未知错误",
    });
  } finally {
    deleting.value = false;
  }
};

/** 关系编辑器：设置保存前要先让它把关系落盘 */
const relationsEditor = ref<InstanceType<typeof CharacterRelationsEditor> | null>(null);

const tabs = computed(() => [
  { id: "basic", label: t("settings.characterInfo.tabs.basic") },
  { id: "prompts", label: t("settings.characterInfo.tabs.prompts") },
  { id: "worldbook", label: t("settings.worldbook.title") },
  { id: "visuals", label: t("settings.characterInfo.tabs.visuals") },
  { id: "avatars", label: t("settings.avatars.title") },
  { id: "clothes", label: t("settings.characterInfo.tabs.clothes") },
  { id: "touch", label: t("settings.characterInfo.tabs.touch") },
  { id: "live2d", label: t("settings.characterInfo.tabs.live2d") },
  { id: "pet", label: t("settings.characterInfo.tabs.pet") },
  { id: "voice", label: t("settings.characterInfo.tabs.voice") },
]);

const resolveFieldOptions = (field: FieldSchema) => {
  const options: FieldOption[] = field.dynamicOptions
    ? field.dynamicOptions()
    : (field.options ?? []);
  return options.filter((option) => !option.visibleIf || option.visibleIf(localSettings.value));
};

// 主对话/桌宠形象共用的选项；按 docs/i18n.md 只翻译标签、不翻译取值
const avatarModeOptions = computed<FieldOption[]>(() => [
  { label: t("settings.characterInfo.avatarModeOptions.live2d"), value: "live2d" },
  { label: t("settings.characterInfo.avatarModeOptions.image"), value: "image" },
]);

const schemas = computed(() =>
  buildSchemas({
    t,
    avatarModeOptions: avatarModeOptions.value,
    cloudVoices: cloudVoices.value,
    installedVoices: installedVoices.value,
  }),
);

// --- Computed Properties ---

const currentTabConfig = computed(() => schemas.value[activeTab.value]);

// voice model 子区域已移除，所有字段统一在主表单渲染
const currentTabFields = computed(() => {
  const fields = currentTabConfig.value || [];
  // 过滤掉当前不可见的字段
  return fields.filter((field) => !field.visibleIf || field.visibleIf(localSettings.value));
});

// --- GSV 六情绪参考语音（自定义 UI；只写草稿，由底部保存统一提交） ---

const fieldModel = (field: FieldSchema) => {
  return computed({
    get: () => {
      let target: any;
      if (field.parent) {
        const parentObj = localSettings.value[field.parent];
        target =
          parentObj && typeof parentObj === "object"
            ? parentObj
            : (localSettings.value[field.parent] = {});
      } else if (field.isVoiceModel) {
        target = ensureVoiceModels();
      } else {
        target = localSettings.value;
      }
      return target[field.key];
    },
    set: (val: any) => {
      const isBlank = val === "" || val === null || val === undefined || Number.isNaN(Number(val));
      const coerced =
        field.type === "number" ? (field.nullable && isBlank ? undefined : Number(val)) : val;
      let target: any;
      if (field.parent) {
        if (
          !localSettings.value[field.parent] ||
          typeof localSettings.value[field.parent] !== "object"
        ) {
          localSettings.value[field.parent] = {};
        }
        target = localSettings.value[field.parent];
      } else if (field.isVoiceModel) {
        target = ensureVoiceModels();
      } else {
        target = localSettings.value;
      }
      target[field.key] = coerced;
    },
  });
};

const clothesList = computed({
  get: () => {
    if (!Array.isArray(localSettings.value.clothes)) {
      localSettings.value.clothes = [];
    }
    return localSettings.value.clothes as Array<{ name: string; prompt: string }>;
  },
  set: (val) => {
    localSettings.value.clothes = val;
  },
});

const addClothesItem = () => {
  if (!Array.isArray(localSettings.value.clothes)) {
    localSettings.value.clothes = [];
  }
  localSettings.value.clothes.push({ name: "", prompt: "" });
};

const removeClothesItem = (idx: number) => {
  if (Array.isArray(localSettings.value.clothes)) {
    localSettings.value.clothes.splice(idx, 1);
  }
};

// --- Watchers & Methods ---

watch(
  () => [props.visible, props.roleId] as const,
  async ([visible]) => {
    const sequence = ++loadSequence;
    if (!visible) {
      loading.value = false;
      loaded.value = false;
      await discardDraft();
      if (sequence === loadSequence) {
        localSettings.value = {};
        baseline.value = "";
        worldbook.value = null;
        worldbookBaseline.value = null;
        worldbookError.value = "";
      }
      return;
    }
    if (!props.roleId) {
      loading.value = false;
      loaded.value = false;
      return;
    }

    // 弹窗可能在 visible 已经为 true 时挂载（关系引导先写意图再打开设置页），
    // 因此这里必须 immediate：否则首帧只渲染空表单，用户点保存有清空角色卡的风险。
    loading.value = true;
    loaded.value = false;
    try {
      await discardDraft();
      if (sequence !== loadSequence || !props.visible) return;
      const roleId = props.roleId;
      const draft = await beginCharacterEdit(roleId);
      if (sequence !== loadSequence || !props.visible || props.roleId !== roleId) {
        await discardCharacterEdit(roleId, draft.edit_id);
        return;
      }
      let loadedBook: Worldbook | null = null;
      let bookError = "";
      try {
        loadedBook = await readCharacterWorldbook(roleId, draft.edit_id);
      } catch (error) {
        bookError = String(error);
      }
      if (sequence !== loadSequence || !props.visible || props.roleId !== roleId) {
        await discardCharacterEdit(roleId, draft.edit_id);
        return;
      }
      worldbook.value = loadedBook;
      worldbookBaseline.value = structuredClone(loadedBook);
      worldbookError.value = bookError;
      editId.value = draft.edit_id;
      editRoleId.value = roleId;
      resourceRevision.value = 0;
      localSettings.value = JSON.parse(JSON.stringify(draft.settings));
      migrateLegacyVoiceModelFields();
      if (!localSettings.value.voice_lang) {
        localSettings.value.voice_lang = "ja";
      }
      // 形象缺省值兜底：老配置缺键时回填 live2d，否则 <select> 显示空白
      if (!localSettings.value.avatar_mode) {
        localSettings.value.avatar_mode = "live2d";
      }
      if (!localSettings.value.avatar_mode_p) {
        localSettings.value.avatar_mode_p = "live2d";
      }
      baseline.value = JSON.stringify(localSettings.value);
      draftClothes.value = [...(props.clothes ?? [])];
      loaded.value = true;
      emit("loaded");
    } catch (e) {
      console.error("Failed to load character settings", e);
      emit("close");
    } finally {
      if (sequence === loadSequence) loading.value = false;
    }
  },
  { immediate: true },
);

// Refresh local voices when the voice tab is shown while the dialog is visible.
watch(
  () => [props.visible, activeTab.value, localSettings.value.tts_type],
  ([visible, tab, ttsType]) => {
    if (visible && tab === "voice") {
      // 本地 TTS 音色:仅 localsbv2api 需要
      if (ttsType === "localsbv2api") {
        void refreshLocalVoices();
      }
      // 云端音色:cosyvoice 下拉随时需要（与 tts_type 无关）
      void refreshCloudVoices();
    }
  },
);

const handleClose = async () => {
  if (saving.value || resourceBusy.value) return;
  if (
    !loading.value &&
    hasChanges.value &&
    !(await dialogStore.confirm(
      t("settings.characterInfo.draft.discardMessage"),
      t("settings.characterInfo.draft.discardTitle"),
    ))
  )
    return;
  ++loadSequence;
  try {
    await discardDraft();
  } catch (error) {
    console.error("清理角色草稿失败", error);
  }
  touchEditorVisible.value = false;
  emit("close");
};

function openCostumeAvatars(name: string) {
  avatarCostume.value = name;
  activeTab.value = "avatars";
}

function onCostumeChanged(change: { oldName: string; newName: string }) {
  if (avatarCostume.value === change.oldName) avatarCostume.value = change.newName;
}

function applySettingsToRuntime() {
  if (!props.roleId) return;
  const runtimeRole = gameStore.gameRoles[props.roleId];
  if (runtimeRole) {
    runtimeRole.scale = Number(localSettings.value.scale ?? 1);
    runtimeRole.offsetX = Number(localSettings.value.offset_x ?? 0);
    runtimeRole.offsetY = Number(localSettings.value.offset_y ?? 0);
    runtimeRole.scaleP = Number(localSettings.value.scale_p ?? 1);
    runtimeRole.offsetXP = Number(localSettings.value.offset_x_p ?? 0);
    runtimeRole.offsetYP = Number(localSettings.value.offset_y_p ?? 0);
    runtimeRole.bubbleTop = Number(localSettings.value.bubble_top ?? 5);
    runtimeRole.bubbleLeft = Number(localSettings.value.bubble_left ?? 20);
    runtimeRole.live2d = localSettings.value.live2d
      ? structuredClone(toRaw(localSettings.value.live2d))
      : null;
    // 形象一并热更，保存后舞台立刻切换（Live2DStage 的 watch 依赖含这两个字段）
    runtimeRole.avatarMode = localSettings.value.avatar_mode ?? null;
    runtimeRole.avatarModeP = localSettings.value.avatar_mode_p ?? null;
    // /chat 与 /pet 互斥，靠这次内存热更避免后端 re-init
    runtimeRole.petFrameless = localSettings.value.pet_frameless ?? false;
    // 触摸区域热更：TouchAreas 直接读 role.bodyPart
    runtimeRole.bodyPart = structuredClone(toRaw(localSettings.value.body_part)) ?? {};
  }
  const role = gameStore.gameRoles[props.roleId];
  if (role) role.clothes = structuredClone(toRaw(localSettings.value.clothes)) ?? [];
}

/** 保存前校验：实际会被加载的那个 variant 的模型文件是否还在。走 getLive2dFilePath 是为了让草稿里的模型按 editId 解析。 */
async function isLive2dModelAvailable(): Promise<boolean> {
  const live2d = localSettings.value.live2d;
  if (!live2d) return false;
  const variant = resolveLive2dVariant(live2d, localSettings.value.clothes_name ?? "");
  if (!variant?.model) return false;
  try {
    await getLive2dFilePath(props.roleId!, variant.model, editId.value);
    return true;
  } catch {
    return false;
  }
}

/** 落盘 + 运行时热更。返回是否成功，成功与否由调用方决定要不要关弹窗。 */
async function persistSettings(): Promise<boolean> {
  if (!loaded.value || !props.roleId || !editId.value || resourceBusy.value || saving.value)
    return false;
  const confirmed = await confirmSaveEffect(
    (save) => t("settings.characterInfo.saveConfirm", { save }),
    t("settings.characterInfo.saveConfirmNoSave"),
  );
  if (!confirmed) return false;
  saving.value = true;
  try {
    if (worldbookChanged.value && !(await worldbookEditor.value?.validate())) {
      activeTab.value = "worldbook";
      return false;
    }
    // 关系先落盘，后保存设置（设置保存会重建人设行，必须读到新关系）。
    await relationsEditor.value?.submit(true);
    // 配了 Live2D 但模型文件取不到时改回静态立绘，否则运行时要先建一套
    // WebGL 上下文、失败之后才退回静态
    if (localSettings.value.live2d && !(await isLive2dModelAvailable())) {
      localSettings.value.avatar_mode = "image";
      localSettings.value.avatar_mode_p = "image";
    }
    await commitCharacterEdit(
      props.roleId,
      editId.value,
      localSettings.value,
      worldbookChanged.value ? { book: worldbook.value } : undefined,
    );
    editId.value = undefined;
    editRoleId.value = undefined;
    applySettingsToRuntime();
    return true;
  } catch (e) {
    console.error("Failed to save settings", e);
    await dialogStore.alert(
      t("settings.characterInfo.draft.saveFailed", {
        error: e instanceof Error ? e.message : String(e),
      }),
    );
    return false;
  } finally {
    saving.value = false;
  }
}

const saveSettings = async () => {
  if (!loaded.value) return;
  if (await persistSettings()) {
    emit("saved");
    emit("close");
  }
};

/** 子编辑器只应用到弹窗草稿，统一由底部保存提交。 */
async function onTouchRegionsApply(value: Record<string, unknown>) {
  if (Object.keys(value).length) localSettings.value.body_part = value;
  else delete localSettings.value.body_part;
  touchEditorVisible.value = false;
}

onUnmounted(() => {
  ++loadSequence;
  loaded.value = false;
  void discardDraft().catch(console.error);
});
</script>

<style scoped>
/* Vertical sidebar: thin custom scrollbar (Webkit + Firefox). */
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
.tab-sidebar-scroll::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.32);
}
</style>

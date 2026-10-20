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
                {{ $t("settings.characterInfo.header.title", { title }) }}
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
                  <!-- 滑动条：推理步数 / 引导强度这类范围有限的数值。
                       设置里还没存过值时显示 defaultValue（后端默认值），而非贴在 min 上。 -->
                  <div v-else-if="field.type === 'range'" class="flex items-center gap-3">
                    <input
                      :id="field.key"
                      type="range"
                      :min="field.min ?? 0"
                      :max="field.max ?? 100"
                      :step="field.step ?? 1"
                      :value="fieldValue(field) ?? field.defaultValue ?? field.min ?? 0"
                      class="accent-brand h-1.5 flex-1 cursor-pointer"
                      @input="onRangeInput(field, $event)"
                    />
                    <span class="w-10 shrink-0 text-right text-sm text-white/80 tabular-nums">
                      {{ formatRangeValue(field) }}
                    </span>
                  </div>
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
                    @change="onSelectChange(field, $event)"
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
                  <div
                    v-else-if="field.type === 'switch'"
                    class="relative flex w-fit items-center"
                    :class="field.disabled ? 'cursor-not-allowed opacity-40' : ''"
                  >
                    <input
                      :id="field.key"
                      v-model="fieldModel(field).value"
                      type="checkbox"
                      class="peer sr-only"
                      :disabled="field.disabled"
                    />
                    <label
                      :for="field.key"
                      class="peer-checked:border-brand peer-checked:bg-brand block h-5 w-9 cursor-pointer rounded-full border border-white/20 bg-white/10 transition-colors"
                    ></label>
                    <div
                      class="pointer-events-none absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white transition-transform peer-checked:translate-x-4"
                    ></div>
                  </div>
                  <!-- 参考音频：导入（复制进应用数据目录）而不是手填路径。
                       Android 上 dialog 返回 content:// URI，后端会经 SAF 落到
                       data/sherpa_onnx_ref_audio/，存的是普通文件路径。 -->
                  <div
                    v-else-if="field.type === 'audio'"
                    class="flex items-center gap-3 rounded-xl border border-white/10 bg-black/20 px-3.5 py-2.5"
                  >
                    <Icon icon="mic" :size="16" class="shrink-0 text-white/40" />
                    <span
                      class="min-w-0 flex-1 truncate text-sm"
                      :class="fieldValue(field) ? 'text-white' : 'text-white/40'"
                      :title="String(fieldValue(field) || '')"
                    >
                      {{
                        fieldValue(field)
                          ? refAudioName(fieldValue(field))
                          : t("settings.sherpa.refNone")
                      }}
                    </span>
                    <button
                      type="button"
                      class="shrink-0 rounded-lg border border-white/10 px-3 py-1.5 text-xs text-white/80 transition-colors hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
                      :disabled="audioImportingKey === field.key"
                      @click="importRefAudio(field)"
                    >
                      {{
                        audioImportingKey === field.key
                          ? t("settings.sherpa.refImporting")
                          : fieldValue(field)
                            ? t("settings.sherpa.refChange")
                            : t("settings.sherpa.refPick")
                      }}
                    </button>
                    <button
                      v-if="fieldValue(field)"
                      type="button"
                      class="shrink-0 rounded-lg border border-white/10 px-2.5 py-1.5 text-xs text-white/80 transition-colors hover:bg-white/10"
                      @click="toggleRefAudioPlayback(field)"
                    >
                      {{
                        audioPlayingKey === field.key
                          ? t("settings.sherpa.refStop")
                          : t("settings.sherpa.refListen")
                      }}
                    </button>
                    <button
                      v-if="fieldValue(field)"
                      type="button"
                      class="shrink-0 rounded-lg border border-white/10 px-2.5 py-1.5 text-xs text-white/60 transition-colors hover:bg-white/10 hover:text-white"
                      @click="
                        stopRefAudioPlayback();
                        fieldModel(field).value = '';
                      "
                    >
                      {{ t("settings.sherpa.refClear") }}
                    </button>
                  </div>
                  <p v-if="field.hint" class="text-[12px] leading-relaxed text-white/40">
                    {{ field.hint }}
                  </p>
                </div>
              </div>

              <!-- 关系（AI 角色视角）：写角色目录下的 relations.yml，独立保存 -->
              <CharacterRelationsEditor
                v-if="activeTab === 'basic' && props.roleId"
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
              :disabled="saving || loading || resourceBusy > 0 || !editId"
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
import { resolveLive2dVariant } from "@/types/live2d";
import { characterEditorKey } from "@/composables/useCharacterEditor";
import * as TtsLocal from "@/api/services/tts/tts-local";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { audioDialogFilters } from "@/utils/dialogFilters";
import { isAndroid, isIOS, isMacOS, isWindows } from "@/utils/platform";
import { decodePathFileName } from "@/utils/path";
import { playLocalAudio, stopCurrentAudio } from "@/utils/mediaUrl";
import { Icon } from "../../base";
import SelectMenu from "../../ui/SelectMenu.vue";
import ExpandableInput from "../../ui/ExpandableInput.vue";
import Live2DSettings from "../character/Live2DSettings.vue";
import AvatarManager from "../character/AvatarManager.vue";
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

const emit = defineEmits(["close", "saved"]);

const activeTab = ref("basic");
const avatarCostume = ref("default");
const loading = ref(false);
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
const draftClothes = ref<Array<{ title: string; avatar: string }>>([]);
const clothesLoading = ref(false);
let clothesSequence = 0;
let loadSequence = 0;
/** 正在导入参考音频的字段 key（同一时刻只会有一个导入在进行）。 */
const audioImportingKey = ref<string | null>(null);
/** 正在试听参考音频的字段 key（同一时刻只会有一个在播放）。 */
const audioPlayingKey = ref<string | null>(null);
provide(characterEditorKey, { editId, busy: resourceBusy, revision: resourceRevision });
const hasChanges = computed(
  () => resourceRevision.value > 0 || JSON.stringify(localSettings.value) !== baseline.value,
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
  installedSherpaModels,
  refreshLocalVoices,
  refreshCloudVoices,
  refreshSherpaModels,
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

const tabs = computed(() => [
  { id: "basic", label: t("settings.characterInfo.tabs.basic") },
  { id: "prompts", label: t("settings.characterInfo.tabs.prompts") },
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

// 下拉项变更后的副作用钩子（v-model 已先写入值，这里只做联动）。
const onSelectChange = (field: FieldSchema, event: Event) => {
  const value = (event.target as HTMLSelectElement).value;
  field.onChange?.(value, localSettings.value);
};

// 主对话/桌宠形象共用的选项；按 docs/i18n.md 只翻译标签、不翻译取值
const avatarModeOptions = computed<FieldOption[]>(() => [
  { label: t("settings.characterInfo.avatarModeOptions.live2d"), value: "live2d" },
  { label: t("settings.characterInfo.avatarModeOptions.image"), value: "image" },
]);

// sherpa-onnx 的「音色」是随模型走的标签（适配器只把它当元数据，真正的说话人
// 固定为 0），所以给固定四档；但模型 config.json 可能带出自定义值（选中模型时
// `applySherpaModelDefaults` 会写进字段），下拉框没有匹配项会显示空白并在切换时
// 丢掉原值，因此把当前值并进选项。空值在后端等同 female，与第一项一致。
const sherpaVoiceOptions = computed<FieldOption[]>(() => {
  const current = String(ensureVoiceModels().sherpa_onnx_voice ?? "").trim();
  const options: FieldOption[] = [
    { label: t("settings.characterInfo.fields.sherpaOnnxVoicesFemale"), value: "female" },
    { label: t("settings.characterInfo.fields.sherpaOnnxVoicesMale"), value: "male" },
    { label: t("settings.characterInfo.fields.sherpaOnnxVoicesChild"), value: "child" },
    { label: t("settings.characterInfo.fields.sherpaOnnxVoicesElderly"), value: "elderly" },
  ];
  if (current && !options.some((option) => option.value === current)) {
    options.unshift({ label: current, value: current });
  }
  return options;
});

/// Linux 桌面上 sherpa-onnx 只剩 CPU（GPU 只能走官方 CUDA 包，安装包 +235 MB，
/// 已按体积取舍移除），把开关置灰并说明；Windows 上则提醒需要 DirectML 版构建
/// （`pnpm tauri:build:win`）。详见 docs/inference-devices.md §9。
const gpuUnsupported = computed(() => !isAndroid() && !isIOS() && !isMacOS() && !isWindows());
const gpuSwitchHint = computed(() => {
  if (gpuUnsupported.value) return t("settings.sherpa.useGpuLinuxHint");
  if (isWindows()) return t("settings.sherpa.useGpuWindowsHint");
  if (isAndroid()) return t("settings.sherpa.useGpuAndroidHint");
  return "";
});

const schemas = computed(() =>
  buildSchemas({
    t,
    avatarModeOptions: avatarModeOptions.value,
    cloudVoices: cloudVoices.value,
    installedVoices: installedVoices.value,
    installedSherpaModels: installedSherpaModels.value,
    applySherpaModelDefaults,
    sherpaVoiceOptions: sherpaVoiceOptions.value,
    gpuUnsupported: gpuUnsupported.value,
    gpuSwitchHint: gpuSwitchHint.value,
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

// 选中 Sherpa 模型后带出它的类型/语言/音色。模型类型是模型自身的属性
// （如 Kokoro 必须用 kokoro），留着默认的 vits 会导致按错误类型加载。
const applySherpaModelDefaults = (modelId: string) => {
  const model = installedSherpaModels.value.find((m) => m.id === modelId);
  if (!model) return;
  const voiceModels = ensureVoiceModels();
  if (model.model_type) voiceModels.sherpa_onnx_model_type = model.model_type;
  if (model.language) voiceModels.sherpa_onnx_lang = model.language;
  if (model.voice) voiceModels.sherpa_onnx_voice = model.voice;
};

/**
 * 历史遗留的 `content://` 参考音频迁移成应用数据目录里的本地 WAV。
 *
 * 早期版本把 dialog 给的路径原样写进设置，Android 上那是 `content://` URI，
 * Rust 的 `std::fs` 读不了 —— 试听会报错，聊天时零样本克隆则静默退化成普通合成。
 * 迁移走已有的导入命令：Rust 侧经 SAF（ContentResolver）拿到真实文件名，并统一
 * 转码成 16-bit PCM WAV。
 *
 * 参考音频在字段 schema 里被标了 `isVoiceModel`，所以新位置是
 * `voice_models.sherpa_onnx_ref_audio_path`；`migrateLegacyVoiceModelFields()` 又会
 * 先于本函数把顶层的遗留键搬进去。两处都要读，结果统一写回 `voice_models` —— 只有
 * 那里会被 Rust 侧的 `build_voice_maker` 喂给 `VoiceMaker`。
 *
 * 迁移后字段值与库里存的不一致，所以底部「保存」会亮起——值要落库才真正生效。
 */
const migrateLegacyRefAudioPath = async () => {
  const roleId = editRoleId.value;
  const voiceModels = ensureVoiceModels();
  const currentRefAudio = () =>
    String(
      localSettings.value.sherpa_onnx_ref_audio_path ??
        voiceModels.sherpa_onnx_ref_audio_path ??
        "",
    ).trim();
  const legacy = currentRefAudio();
  if (!legacy.startsWith("content://")) return;
  const imported = await TtsLocal.migrateSherpaRefAudio(legacy);
  if (!imported) return;
  // 异步期间用户可能切了角色、或者自己改掉了这一项，只在仍是原值时写回。
  if (editRoleId.value !== roleId) return;
  if (currentRefAudio() !== legacy) return;
  voiceModels.sherpa_onnx_ref_audio_path = imported.path;
};

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
        field.type === "number" || field.type === "range"
          ? field.nullable && isBlank
            ? undefined
            : Number(val)
          : val;
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

/** 读取字段当前值；模板里要多次读同一个字段时，避免重复 new computed。 */
const fieldValue = (field: FieldSchema): any => fieldModel(field).value;

/** 滑块右侧的数值：没存过时显示 defaultValue，小数位跟随 step（4 / 1.0）。 */
const formatRangeValue = (field: FieldSchema): string => {
  const raw = fieldValue(field) ?? field.defaultValue ?? field.min ?? 0;
  const decimals = String(field.step ?? "1").includes(".") ? 1 : 0;
  return Number(raw).toFixed(decimals);
};

/** range 输入框给的是字符串，必须显式转数字，否则会被存成 "4"。 */
const onRangeInput = (field: FieldSchema, event: Event): void => {
  const value = Number((event.target as HTMLInputElement).value);
  if (!Number.isFinite(value)) return;
  fieldModel(field).value = value;
};

// 兼容历史遗留的取值：早期版本存的是用户手填的路径，Android 上还可能是
// `content://` URI（末段是 percent-encoded 的 document id），统一交给公共
// util 解码后取文件名，避免把一长串 URI 显示给用户。
const refAudioName = (value: unknown): string => decodePathFileName(String(value ?? ""));

/**
 * 导入零样本克隆的参考音频。
 *
 * 存进字段的是后端复制到应用数据目录后的本地路径（不是 dialog 给的原始路径）：
 * Android 的 `content://` URI 无法被 Rust 的 `std::fs` 读取，桌面端原始路径也会
 * 因用户移动/删除源文件而失效。后端还会顺手把音频统一转成 sherpa 要的 16-bit
 * PCM WAV，所以 mp3/flac/m4a 等格式都能直接导入。
 */
async function importRefAudio(field: FieldSchema): Promise<void> {
  try {
    const selected = await open({ multiple: false, filters: audioDialogFilters() });
    if (typeof selected !== "string") return;
    audioImportingKey.value = field.key;
    const imported = await TtsLocal.importSherpaRefAudio(selected);
    stopRefAudioPlayback();
    fieldModel(field).value = imported.path;
    uiStore.showSuccess({
      title: t("settings.sherpa.refLoaded"),
      message: imported.file_name,
    });
  } catch (error: any) {
    const rawMessage = typeof error === "string" ? error : error?.message || String(error);
    console.error("[SettingsCharacterInfo] 导入参考音频失败:", error);
    const codeMessages: Record<string, string> = {
      SHERPA_REF_AUDIO_UNSUPPORTED: t("settings.sherpa.refImportUnsupported"),
      SHERPA_REF_AUDIO_OPUS: t("settings.sherpa.refImportOpus"),
      SHERPA_REF_AUDIO_TOO_LONG: t("settings.sherpa.refImportTooLong"),
    };
    uiStore.showError({
      title: t("settings.sherpa.refImportFailed"),
      message: codeMessages[rawMessage] ?? rawMessage,
    });
  } finally {
    audioImportingKey.value = null;
  }
}

/** 停止参考音频试听（切换字段、重新导入、离开页面时都要停）。 */
function stopRefAudioPlayback(): void {
  stopCurrentAudio();
  audioPlayingKey.value = null;
}

/**
 * 试听已导入的参考音频。
 *
 * 导入时后端统一转成了 WAV（mp3/m4a 等会被转码），这里让用户能直接确认转码
 * 结果是否正常。播放走公共 `playLocalAudio`：Android 上先整个 fetch 成 blob，
 * 绕开 asset 协议对大文件的分块截断。
 */
async function toggleRefAudioPlayback(field: FieldSchema): Promise<void> {
  const path = String(fieldValue(field) ?? "").trim();
  if (!path) return;
  if (audioPlayingKey.value === field.key) {
    stopRefAudioPlayback();
    return;
  }
  stopRefAudioPlayback();
  try {
    const audio = await playLocalAudio(path);
    audioPlayingKey.value = field.key;
    const done = () => {
      if (audioPlayingKey.value === field.key) audioPlayingKey.value = null;
    };
    // 用 addEventListener 而不是覆盖 onended：playLocalAudio 自己也挂了 onended。
    audio.addEventListener("ended", done);
    audio.addEventListener("error", done);
  } catch (error: any) {
    audioPlayingKey.value = null;
    console.error("[SettingsCharacterInfo] 试听参考音频失败:", error);
    uiStore.showError({
      title: t("settings.sherpa.refPlayFailed"),
      message: typeof error === "string" ? error : error?.message || String(error),
    });
  }
}

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
  async ([newVal]) => {
    const sequence = ++loadSequence;
    if (!newVal) {
      await discardDraft();
      return;
    }
    if (newVal && props.roleId) {
      loading.value = true;
      try {
        await discardDraft();
        if (sequence !== loadSequence || !props.visible) return;
        const roleId = props.roleId;
        const draft = await beginCharacterEdit(roleId);
        if (sequence !== loadSequence || !props.visible || props.roleId !== roleId) {
          await discardCharacterEdit(roleId, draft.edit_id);
          return;
        }
        editId.value = draft.edit_id;
        editRoleId.value = roleId;
        resourceRevision.value = 0;
        localSettings.value = JSON.parse(JSON.stringify(draft.settings));
        migrateLegacyVoiceModelFields();
        void migrateLegacyRefAudioPath();
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
      } catch (e) {
        console.error("Failed to load character settings", e);
        emit("close");
      } finally {
        if (sequence === loadSequence) loading.value = false;
      }
    }
  },
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
      // Sherpa-ONNX 本地模型下拉:仅 sherpa-onnx 需要
      if (ttsType === "sherpa-onnx") {
        void refreshSherpaModels();
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
  if (!props.roleId || !editId.value || resourceBusy.value || saving.value) return false;
  saving.value = true;
  try {
    // 配了 Live2D 但模型文件取不到时改回静态立绘，否则运行时要先建一套
    // WebGL 上下文、失败之后才退回静态
    if (localSettings.value.live2d && !(await isLive2dModelAvailable())) {
      localSettings.value.avatar_mode = "image";
      localSettings.value.avatar_mode_p = "image";
    }
    await commitCharacterEdit(props.roleId, editId.value, localSettings.value);
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
  stopRefAudioPlayback();
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

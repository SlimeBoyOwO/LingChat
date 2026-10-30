<template>
  <Transition name="modal">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm"
      @click="handleClose"
    >
      <div
        class="glass-effect flex max-h-[85dvh] w-full max-w-2xl flex-col overflow-hidden rounded-3xl text-white"
        @click.stop
      >
        <div class="flex items-center justify-between border-b border-white/10 bg-white/5 p-6">
          <div class="flex items-center gap-4">
            <div
              class="flex h-12 w-12 items-center justify-center rounded-xl bg-white/10 shadow-inner"
            >
              <User :size="22" />
            </div>
            <div>
              <h2 class="m-0 text-xl font-bold drop-shadow-[0_2px_4px_rgba(0,0,0,0.3)]">
                {{
                  identityId
                    ? $t("settings.identity.editorTitle")
                    : $t("settings.identity.editorCreateTitle")
                }}
              </h2>
              <p class="m-0 text-sm text-white/50">
                {{ $t("settings.identity.editorSubtitle") }}
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

        <div class="flex-1 space-y-4 overflow-y-auto p-6">
          <div v-if="loading" class="flex h-32 items-center justify-center">
            <div
              class="h-10 w-10 animate-spin rounded-full border-3 border-white/10 border-t-[#5e72e4]"
            ></div>
          </div>

          <template v-else-if="form">
            <div class="flex flex-col gap-1.5">
              <label class="text-xs font-medium text-white/60">{{
                $t("settings.identity.fieldName")
              }}</label>
              <input v-model="form.name" class="glass-input" />
            </div>
            <div class="flex flex-col gap-1.5">
              <label class="text-xs font-medium text-white/60">{{
                $t("settings.identity.fieldSubtitle")
              }}</label>
              <input v-model="form.subtitle" class="glass-input" />
            </div>
            <div class="flex flex-col gap-1.5">
              <label class="text-xs font-medium text-white/60">{{
                $t("settings.identity.fieldPrompt")
              }}</label>
              <textarea v-model="form.prompt" rows="4" class="glass-input"></textarea>
              <p class="text-[11px] leading-relaxed text-white/40">
                {{ $t("settings.identity.promptPublicHint") }}
              </p>
            </div>

            <!-- 关系：我和各个 AI 角色 / 我的其他身份 -->
            <div id="relation-block-identity" class="flex flex-col gap-2">
              <label class="text-xs font-medium text-white/60">{{
                $t("settings.identity.fieldRelations")
              }}</label>
              <p class="text-[11px] leading-relaxed text-white/40">
                {{ $t("settings.identity.relationsHint") }}
              </p>
              <div v-for="(row, idx) in relationRows" :key="idx" class="flex items-center gap-2">
                <SelectMenu
                  v-model="row.target"
                  class="w-40 shrink-0"
                  :options="relationOptionsFor(row)"
                />
                <ExpandableInput v-model="row.text" class="glass-input flex-1" />
                <button class="btn-sm" @click="removeRelationRow(idx)">×</button>
              </div>
              <button class="btn-sm" @click="addRelationRow">
                {{ $t("settings.identity.addRelation") }}
              </button>
            </div>
          </template>
        </div>

        <div
          class="flex items-center justify-between gap-3 border-t border-white/10 bg-white/5 p-4"
        >
          <button
            v-if="identityId"
            class="cursor-pointer rounded-[20px] border border-red-400/30 bg-red-500/15 px-4 py-2 text-sm font-medium text-red-200 transition-all duration-200 hover:bg-red-500/30 disabled:cursor-not-allowed disabled:opacity-50"
            :disabled="saving || deleting"
            @click="removeIdentity"
          >
            {{ $t("settings.identity.delete") }}
          </button>
          <span v-else></span>
          <div class="flex gap-3">
            <button
              class="cursor-pointer rounded-[20px] border-none bg-white/10 px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:bg-white/20"
              @click="handleClose"
            >
              {{ $t("settings.identity.cancel") }}
            </button>
            <button
              class="cursor-pointer rounded-[20px] border-none bg-[#5e72e4] px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:enabled:-translate-y-px hover:enabled:bg-[#4a5acf] disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="saving || deleting || loading || !form"
              @click="submit"
            >
              {{ $t("settings.identity.save") }}
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
import { User } from "@lucide/vue";

import SelectMenu from "../../ui/SelectMenu.vue";
import ExpandableInput from "../../ui/ExpandableInput.vue";
import { confirmSaveEffect } from "../../../composables/usePreSaveConfirm";
import {
  useRelationEditor,
  type RelationTargetOption,
} from "../../../composables/useRelationEditor";
import {
  deletePlayerIdentity,
  emptyPlayerIdentity,
  getPlayerIdentity,
  meKey,
  savePlayerIdentity,
  type PlayerIdentity,
} from "../../../api/services/player-identity";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { useDialogStore } from "../../../stores/modules/ui/dialog";

const props = defineProps<{
  visible: boolean;
  /** 要编辑的身份 id；null 表示新建 */
  identityId: string | null;
  /** 关系目标（全部角色 + 其他身份），由设置页统一提供 */
  targets: RelationTargetOption[];
}>();

const emit = defineEmits<{ close: []; saved: [id?: string] }>();

const { t } = useI18n();
const uiStore = useUIStore();
const dialogStore = useDialogStore();

const form = ref<PlayerIdentity | null>(null);
const loading = ref(false);
const saving = ref(false);
const deleting = ref(false);

// 不让身份把自己列为「我的身份」关系目标
const relationTargets = computed<RelationTargetOption[]>(() =>
  props.identityId
    ? props.targets.filter((item) => item.value !== meKey(props.identityId as string))
    : props.targets,
);

const {
  rows: relationRows,
  syncRows,
  addRow,
  removeRow: removeRelationRow,
  fold,
  optionsFor: relationOptionsFor,
} = useRelationEditor(relationTargets);

const addRelationRow = () => {
  addRow(relationTargets.value[0]?.value ?? "");
};

let loadSeq = 0;
watch(
  () => [props.visible, props.identityId] as const,
  async ([visible, identityId]) => {
    const seq = ++loadSeq;
    if (!visible) {
      form.value = null;
      syncRows({});
      loading.value = false;
      return;
    }
    loading.value = true;
    try {
      const identity = identityId ? await getPlayerIdentity(identityId) : emptyPlayerIdentity();
      if (seq !== loadSeq) return;
      form.value = identity ?? emptyPlayerIdentity();
      syncRows(form.value.relations);
    } catch (e) {
      console.error("读取身份失败:", e);
      uiStore.showError({
        title: t("settings.identity.msg.saveFailTitle"),
        message: typeof e === "string" ? e : (e as Error).message,
      });
      emit("close");
    } finally {
      if (seq === loadSeq) loading.value = false;
    }
  },
  { immediate: true },
);

const handleClose = () => {
  if (saving.value || deleting.value) return;
  emit("close");
};

const submit = async () => {
  const current = form.value;
  if (!current) return;
  if (!current.name.trim()) {
    uiStore.showError({
      title: t("settings.identity.msg.saveFailTitle"),
      message: t("settings.identity.msg.nameRequired"),
    });
    return;
  }

  const confirmed = await confirmSaveEffect(
    (save) => t("settings.identity.saveConfirm", { save }),
    t("settings.identity.saveConfirmNoSave"),
  );
  if (!confirmed) return;

  saving.value = true;
  try {
    const saved = await savePlayerIdentity({ ...current, relations: fold() });
    emit("saved", saved.id);
    uiStore.showSuccess({
      title: t("settings.identity.msg.savedTitle"),
      message: t("settings.identity.msg.savedMsg"),
    });
  } catch (e: any) {
    uiStore.showError({
      title: t("settings.identity.msg.saveFailTitle"),
      message: typeof e === "string" ? e : e.message || t("settings.identity.msg.saveFailTitle"),
    });
  } finally {
    saving.value = false;
  }
};

const removeIdentity = async () => {
  const current = form.value;
  if (!current) return;
  const confirmed = await dialogStore.confirm(
    t("settings.identity.msg.deleteConfirm", { name: current.name }),
  );
  if (!confirmed) return;
  deleting.value = true;
  try {
    await deletePlayerIdentity(current.id);
    emit("saved");
  } catch (e: any) {
    uiStore.showError({
      title: t("settings.identity.msg.deleteFailTitle"),
      message: typeof e === "string" ? e : e.message || t("settings.identity.msg.deleteFailTitle"),
    });
  } finally {
    deleting.value = false;
  }
};
</script>

<style scoped>
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
</style>

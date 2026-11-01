<template>
  <div id="relation-block-role" class="space-y-3 rounded-xl border border-white/10 bg-black/20 p-4">
    <div class="min-w-0">
      <h3 class="text-sm font-bold text-white/80">
        {{ $t("settings.characterInfo.relations.title") }}
      </h3>
      <p class="mt-1 text-[11px] leading-relaxed text-white/40">
        {{ $t("settings.characterInfo.relations.hint") }}
      </p>
    </div>
    <div v-if="relationsLoading" class="text-xs text-white/40">
      {{ $t("settings.shared.loading") }}
    </div>

    <template v-else>
      <div v-for="(row, idx) in relationRows" :key="idx" class="flex items-center gap-2">
        <SelectMenu v-model="row.target" class="w-44 shrink-0" :options="optionsFor(row)" />
        <ExpandableInput v-model="row.text" class="glass-input flex-1" />
        <!-- 我的身份行：该角色对 ta 的初始好感度覆写，留空跟随角色卡默认 -->
        <input
          v-if="row.target.startsWith('me:')"
          :value="row.affection ?? ''"
          type="number"
          min="-100"
          max="100"
          class="glass-input w-22 shrink-0 text-center"
          :placeholder="$t('settings.characterInfo.relations.affectionPlaceholder')"
          :title="$t('settings.characterInfo.relations.affectionHint')"
          @input="setAffection(row, ($event.target as HTMLInputElement).value)"
        />
        <button class="btn-sm btn-sm-ghost" @click="removeRow(idx)">×</button>
      </div>
      <button class="btn-sm btn-sm-ghost" @click="addRow()">
        {{ $t("settings.identity.addRelation") }}
      </button>
      <p class="text-[11px] leading-relaxed text-white/35">
        {{ $t("settings.characterInfo.relations.affectionHint") }}
      </p>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { characterGetAll } from "@/api/services/character";
import type { Character as ApiCharacter } from "@/types";
import {
  aiKey,
  getRoleRelations,
  listPlayerIdentities,
  meKey,
  saveRoleRelations,
  type PlayerIdentitySummary,
} from "@/api/services/player-identity";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useRelationEditor, type RelationTargetOption } from "@/composables/useRelationEditor";
import SelectMenu from "../../../ui/SelectMenu.vue";
import ExpandableInput from "../../../ui/ExpandableInput.vue";

const props = defineProps<{
  roleId: number;
  /** 当前角色目录名，用于把角色自己从关系目标里排掉 */
  characterFolder: string;
}>();

const { t } = useI18n();
const uiStore = useUIStore();

const relationTargets = ref<RelationTargetOption[]>([]);
const {
  rows: relationRows,
  syncRows,
  addRow,
  removeRow,
  fold,
  foldAffections,
  setAffection,
  optionsFor,
} = useRelationEditor(relationTargets, (key) =>
  t("settings.characterInfo.relations.missing", { key }),
);

const relationsLoading = ref(false);
const relationsSaving = ref(false);
/** 关系是否已成功加载；未成功时提交跳过，避免把关系清空 */
const relationsLoaded = ref(false);

/** 下拉要覆盖全部角色，不只当前分页。 */
const fetchAllRelationCharacters = async (): Promise<ApiCharacter[]> => {
  const pageSize = 100;
  const first = await characterGetAll(1, pageSize);
  const items = [...first.items];
  const totalPages = Math.min(first.total_pages || 1, 50);
  for (let page = 2; page <= totalPages; page += 1) {
    const next = await characterGetAll(page, pageSize);
    if (!next.items.length) break;
    items.push(...next.items);
  }
  return items;
};

let loadSeq = 0;

const loadRelations = async (roleId: number) => {
  const seq = ++loadSeq;
  relationsLoading.value = true;
  try {
    const [chars, identities] = await Promise.all([
      fetchAllRelationCharacters().catch(() => [] as ApiCharacter[]),
      listPlayerIdentities().catch(() => [] as PlayerIdentitySummary[]),
    ]);
    if (seq !== loadSeq) return;

    const selfFolder = props.characterFolder;
    relationTargets.value = [
      ...chars
        .filter((c) => !!c.resource_folder && c.resource_folder !== selfFolder)
        .map((c) => ({
          value: aiKey(c.resource_folder as string),
          label: c.name,
          group: t("settings.identity.targetAi"),
        })),
      ...identities.map((i) => ({
        value: meKey(i.id),
        label: i.name,
        group: t("settings.identity.targetMe"),
      })),
    ];

    const saved = await getRoleRelations(roleId);
    if (seq !== loadSeq) return;
    syncRows(saved.relations, saved.affections);
    relationsLoaded.value = true;
  } catch (e: any) {
    if (seq !== loadSeq) return;
    console.error("读取角色关系失败:", e);
    uiStore.showError({
      title: t("settings.characterInfo.relations.msg.loadFailTitle"),
      message: typeof e === "string" ? e : e?.message || "",
    });
    relationRows.value = [];
    relationsLoaded.value = false;
  } finally {
    if (seq === loadSeq) relationsLoading.value = false;
  }
};

/** 由外部保存流程调用：设置保存会重建人设行，必须先把关系落盘。 */
const submit = async (silent = false): Promise<void> => {
  if (!relationsLoaded.value || !props.roleId) return;
  relationsSaving.value = true;
  try {
    await saveRoleRelations(props.roleId, fold(), foldAffections());
    if (!silent) {
      uiStore.showSuccess({
        title: t("settings.characterInfo.relations.msg.savedTitle"),
        message: t("settings.characterInfo.relations.msg.savedMsg"),
      });
    }
  } finally {
    relationsSaving.value = false;
  }
};

defineExpose({ submit, relationsSaving });

// roleId 决定取哪张卡的关系，characterFolder 决定要不要排除自己；两者变化都要重读
watch(
  () => [props.roleId, props.characterFolder] as const,
  ([roleId]) => {
    if (roleId) void loadRelations(roleId);
  },
  { immediate: true },
);
</script>

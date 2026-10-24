<template>
  <div id="relation-block-role" class="space-y-3 rounded-xl border border-white/10 bg-black/20 p-4">
    <div class="flex items-start justify-between gap-3">
      <div class="min-w-0">
        <h3 class="text-sm font-bold text-white/80">
          {{ $t("settings.characterInfo.relations.title") }}
        </h3>
        <p class="mt-1 text-[11px] leading-relaxed text-white/40">
          {{ $t("settings.characterInfo.relations.hint") }}
        </p>
      </div>
      <button
        class="btn-sm btn-sm-ghost shrink-0"
        :disabled="relationsSaving || relationsLoading"
        @click="saveRelations"
      >
        {{
          relationsSaving
            ? $t("settings.shared.loading")
            : $t("settings.characterInfo.relations.save")
        }}
      </button>
    </div>
    <div v-if="relationsLoading" class="text-xs text-white/40">
      {{ $t("settings.shared.loading") }}
    </div>

    <template v-else>
      <div v-for="(row, idx) in relationRows" :key="idx" class="flex items-center gap-2">
        <SelectMenu v-model="row.target" class="w-44 shrink-0" :options="relationOptionsFor(row)" />
        <ExpandableInput v-model="row.text" class="glass-input flex-1" />
        <button class="btn-sm btn-sm-ghost" @click="removeRelationRow(idx)">×</button>
      </div>
      <button class="btn-sm btn-sm-ghost" @click="addRelationRow">
        {{ $t("settings.identity.addRelation") }}
      </button>
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
import SelectMenu from "../../../ui/SelectMenu.vue";
import ExpandableInput from "../../../ui/ExpandableInput.vue";

const props = defineProps<{
  roleId: number;
  /** 当前角色目录名，用于把角色自己从关系目标里排掉 */
  characterFolder: string;
}>();

interface RelationTarget {
  value: string;
  label: string;
  group: "ai" | "me";
}

const { t } = useI18n();
const uiStore = useUIStore();

const relationTargets = ref<RelationTarget[]>([]);
/** 编辑中的关系行（目标 + 文本）；保存时折成 {关系键: 文本} 映射 */
const relationRows = ref<{ target: string; text: string }[]>([]);
const relationsLoading = ref(false);
const relationsSaving = ref(false);

/** 目标已不存在时保留原值并标注 */
const isMissingTarget = (target: string) =>
  !!target && !relationTargets.value.some((item) => item.value === target);

/** 下拉选项：其他行已选过的目标置灰不可再选（本行当前值除外） */
const relationOptionsFor = (row: { target: string }) => {
  const used = new Set(relationRows.value.map((r) => r.target).filter(Boolean));
  const options: { label: string; value: string; group?: string; disabled: boolean }[] =
    relationTargets.value.map((item) => ({
      label: item.label,
      value: item.value,
      group:
        item.group === "ai" ? t("settings.identity.targetAi") : t("settings.identity.targetMe"),
      disabled: item.value !== row.target && used.has(item.value),
    }));
  // 目标已被删除时保留原值，避免静默丢数据
  if (isMissingTarget(row.target)) {
    options.unshift({
      label: t("settings.characterInfo.relations.missing", { key: row.target }),
      value: row.target,
      disabled: false,
    });
  }
  return options;
};

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
          group: "ai" as const,
        })),
      ...identities.map((i) => ({
        value: meKey(i.id),
        label: i.name,
        group: "me" as const,
      })),
    ];

    const saved = await getRoleRelations(roleId);
    if (seq !== loadSeq) return;
    relationRows.value = Object.entries(saved || {})
      .filter(([target, text]) => !!target && !!text)
      .map(([target, text]) => ({ target, text }));
  } catch (e: any) {
    if (seq !== loadSeq) return;
    console.error("读取角色关系失败:", e);
    uiStore.showError({
      title: t("settings.characterInfo.relations.msg.loadFailTitle"),
      message: typeof e === "string" ? e : e?.message || "",
    });
    relationRows.value = [];
  } finally {
    if (seq === loadSeq) relationsLoading.value = false;
  }
};

const addRelationRow = () => {
  // 目标默认留空（不预选角色）；保存时空目标的行走丢弃逻辑
  relationRows.value.push({ target: "", text: "" });
};

const removeRelationRow = (idx: number) => {
  relationRows.value.splice(idx, 1);
};

const saveRelations = async () => {
  relationsSaving.value = true;
  try {
    // 空目标 / 空文本直接丢弃（后端也会再过滤一次）
    const map: Record<string, string> = {};
    for (const row of relationRows.value) {
      if (row.target && row.text.trim()) map[row.target] = row.text.trim();
    }
    await saveRoleRelations(props.roleId, map);
    uiStore.showSuccess({
      title: t("settings.characterInfo.relations.msg.savedTitle"),
      message: t("settings.characterInfo.relations.msg.savedMsg"),
    });
  } catch (e: any) {
    uiStore.showError({
      title: t("settings.characterInfo.relations.msg.saveFailTitle"),
      message: typeof e === "string" ? e : e?.message || "",
    });
  } finally {
    relationsSaving.value = false;
  }
};

// roleId 决定取哪张卡的关系，characterFolder 决定要不要排除自己；两者变化都要重读
watch(
  () => [props.roleId, props.characterFolder] as const,
  ([roleId]) => {
    if (roleId) void loadRelations(roleId);
  },
  { immediate: true },
);
</script>

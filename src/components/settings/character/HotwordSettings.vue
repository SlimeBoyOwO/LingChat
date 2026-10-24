<template>
  <div class="space-y-4">
    <!-- 模型不吃热词：当前模型的两条链路（即时热词 / prompt 偏置）都不支持 -->
    <p
      v-if="modelCapability && !modelCapability.supportsHotwords"
      class="rounded-xl border border-amber-400/30 bg-amber-500/10 px-3 py-2 text-[15px] leading-relaxed text-amber-200"
    >
      ⚠
      {{
        $t("settings.characterInfo.hotword.modelUnsupported", { model: modelCapability.modelId })
      }}
    </p>

    <!-- 条数超限同样会被后端整表拒（用户粘大段文本批量新增时最容易撞上） -->
    <p
      v-if="draft.hotwords.length > MAX_HOTWORDS_PER_ROLE"
      class="rounded-xl border border-red-400/30 bg-red-500/10 px-3 py-2 text-[12px] leading-relaxed text-red-200"
    >
      {{
        $t("settings.characterInfo.hotword.tooMany", {
          count: draft.hotwords.length,
          max: MAX_HOTWORDS_PER_ROLE,
        })
      }}
    </p>

    <div class="flex flex-wrap items-start justify-between gap-3">
      <div class="min-w-0">
        <div class="flex flex-wrap items-center gap-2">
          <h3 v-if="scope === 'role'" class="text-lg font-semibold text-white">
            {{ $t("settings.characterInfo.tabs.hotword") }}
          </h3>
          <span
            class="rounded-full border border-white/10 bg-white/5 px-2.5 py-0.5 text-[12px] text-white/50"
          >
            {{
              $t("settings.characterInfo.hotword.count", {
                count: draft.hotwords.length,
                max: MAX_HOTWORDS_PER_ROLE,
              })
            }}
          </span>
        </div>
        <p v-if="scope === 'role'" class="mt-1 text-[13px] leading-relaxed text-white/50">
          {{ $t("settings.characterInfo.hotword.roleHint") }}
        </p>
      </div>
      <div class="flex flex-wrap items-center justify-end gap-2">
        <template v-if="batchMode">
          <button
            class="cursor-pointer rounded-lg border border-white/15 bg-white/10 px-3.5 py-2 text-sm text-white transition-colors hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-40"
            :disabled="draft.hotwords.length === 0"
            @click="toggleAll"
          >
            {{
              allSelected
                ? $t("settings.hotwordBatch.clearSelection")
                : $t("settings.hotwordBatch.selectAll")
            }}
          </button>
          <button
            class="cursor-pointer rounded-lg border border-red-400/40 bg-red-500/20 px-3.5 py-2 text-sm text-red-200 transition-colors hover:bg-red-500/30 disabled:cursor-not-allowed disabled:opacity-40"
            :disabled="selectedCount === 0"
            @click="deleteSelected"
          >
            {{ $t("settings.hotwordBatch.deleteSelected", { count: selectedCount }) }}
          </button>
          <button
            class="cursor-pointer rounded-lg border border-white/15 bg-white/10 px-3.5 py-2 text-sm text-white transition-colors hover:bg-white/20"
            @click="toggleBatchMode"
          >
            {{ $t("settings.hotwordBatch.exit") }}
          </button>
        </template>
        <template v-else>
          <button
            class="cursor-pointer rounded-lg border border-white/15 bg-white/10 px-3.5 py-2 text-sm text-white transition-colors hover:bg-white/20"
            @click="showPicker = true"
          >
            {{ $t("settings.asr.hotwordLibrary.pickButton") }}
          </button>
          <button
            class="cursor-pointer rounded-lg border border-white/15 bg-white/10 px-3.5 py-2 text-sm text-white transition-colors hover:bg-white/20"
            @click="showImport = true"
          >
            {{ $t("settings.hotwordImport.entry") }}
          </button>
          <button
            v-if="draft.hotwords.length > 0"
            class="cursor-pointer rounded-lg border border-white/15 bg-white/10 px-3.5 py-2 text-sm text-white transition-colors hover:bg-white/20"
            @click="toggleBatchMode"
          >
            {{ $t("settings.hotwordBatch.entry") }}
          </button>
        </template>
      </div>
    </div>

    <div
      v-if="draft.hotwords.length > 0"
      class="flex items-center gap-2 px-2 text-[13px] text-white/40"
    >
      <span v-if="batchMode" class="size-4 shrink-0"></span>
      <span class="flex-1">{{ $t("settings.characterInfo.hotword.colText") }}</span>
      <span class="w-24 shrink-0">{{ $t("settings.characterInfo.hotword.colWeight") }}</span>
      <span class="w-6 shrink-0"></span>
    </div>

    <div
      v-for="(item, idx) in draft.hotwords"
      :key="idx"
      class="rounded-xl border border-white/10 bg-white/5 p-2"
    >
      <div class="flex items-center gap-2">
        <input
          v-if="batchMode"
          type="checkbox"
          class="size-4 shrink-0 cursor-pointer accent-[#5e72e4]"
          :checked="selected.has(item)"
          @change="toggleRow(item)"
        />
        <input
          v-model="item.text"
          type="text"
          class="hotword-control flex-1 rounded-xl border bg-black/20 px-3 py-2 text-sm text-white transition-all duration-200 outline-none"
          :class="isHotwordTextOverlong(item.text) ? 'border-red-400/60' : 'border-white/10'"
          @change="commitText(item)"
        />
        <select
          v-model.number="item.weight"
          class="hotword-control w-24 shrink-0 rounded-xl border border-white/10 bg-black/20 px-2 py-2 text-sm text-white transition-all duration-200 outline-none"
          @change="clampWeight(item)"
        >
          <option
            v-for="w in HOTWORD_WEIGHT_OPTIONS"
            :key="w"
            :value="w"
            class="bg-[#333] text-white"
          >
            {{ weightLabel(w) }}
          </option>
        </select>
        <button
          :title="$t('settings.characterInfo.hotword.delete')"
          class="h-6 w-6 shrink-0 cursor-pointer rounded-full border-none bg-red-500/20 text-xs text-red-400 transition-colors hover:bg-red-500/40"
          @click="removeHotword(idx)"
        >
          ×
        </button>
      </div>
      <!-- 超长条目会被后端整表拒；提示按字符数 / 片段数两套口径走。 -->
      <p v-if="hotwordLimitKind(item.text) !== 'ok'" class="mt-1 text-[12px] text-red-300">
        {{
          hotwordLimitKind(item.text) === "segments"
            ? $t("settings.characterInfo.hotword.tooLongSegments", {
                count: hotwordSegmentCount(item.text),
                max: MAX_HOTWORD_SEGMENTS,
              })
            : $t("settings.characterInfo.hotword.tooLongChars", {
                count: hotwordCharCount(item.text),
                max: MAX_HOTWORD_CHARS,
              })
        }}
      </p>
    </div>

    <div v-if="draft.hotwords.length === 0" class="py-8 text-center text-sm text-white/40">
      {{ $t("settings.characterInfo.hotword.empty") }}
    </div>

    <button
      type="button"
      class="hover:border-brand/50 w-full cursor-pointer rounded-xl border border-dashed border-white/20 bg-white/5 px-3 py-2.5 text-sm text-white/60 transition-colors hover:bg-white/10 hover:text-white"
      @click="addHotword"
    >
      + {{ $t("settings.characterInfo.hotword.add") }}
    </button>

    <p
      class="rounded-xl border border-white/10 bg-white/5 px-3 py-2 text-[13px] leading-relaxed text-white/50"
    >
      {{
        scope === "global"
          ? $t("settings.asr.globalHotword.dialog.weightHint")
          : $t("settings.characterInfo.hotword.roleAppendHint")
      }}
    </p>

    <HotwordImportDialog
      :visible="showImport"
      :existing="draft.hotwords"
      :capacity="MAX_HOTWORDS_PER_ROLE"
      :on-save="handleImportSave"
      @close="showImport = false"
    />

    <!-- 选择器嵌在宿主弹窗之上：宿主可能是角色设置弹窗（z-50）或全局弹窗（z-[10000]） -->
    <HotwordLibraryPicker
      :visible="showPicker"
      :existing="draft.hotwords"
      :capacity="MAX_HOTWORDS_PER_ROLE"
      @close="showPicker = false"
      @add="appendFromLibrary"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  MAX_HOTWORD_CHARS,
  MAX_HOTWORD_SEGMENTS,
  MAX_HOTWORDS_PER_ROLE,
  hotwordCharCount,
  hotwordLimitKind,
  hotwordSegmentCount,
  isHotwordTextOverlong,
  type LibraryWord,
  type RoleHotword,
  type RoleHotwordDraft,
} from "@/api/services/hotword";
import HotwordImportDialog from "@/components/settings/asr/HotwordImportDialog.vue";
import HotwordLibraryPicker from "@/components/settings/asr/HotwordLibraryPicker.vue";
import { useUIStore } from "@/stores/modules/ui/ui";
import {
  HOTWORD_DEFAULT_WEIGHT,
  HOTWORD_WEIGHT_OPTIONS,
  clampRowWeight,
  commitRowText,
  dedupeRows,
} from "@/utils/hotwordRows";
import type { ImportRow } from "@/utils/hotwordImport";

const props = defineProps<{
  /** 角色侧是真实 roleId；全局侧固定 0 占位。 */
  roleId: number;
  /** 当前实际生效的模型能力；`null` = 清单尚未加载，不提示。 */
  modelCapability: { modelId: string; supportsHotwords: boolean } | null;
  /** 草稿：编辑只进这里，提交由父组件负责。 */
  modelValue: RoleHotwordDraft;
  /** 角色/全局共用本组件，但保存命令不同，由宿主注入。 */
  saveHotwords: (rows: RoleHotword[]) => Promise<void>;
  /** 控制标题与底部提示的文案；全局弹窗的标题在宿主头部。 */
  scope: "role" | "global";
}>();

// v-model 的草稿**就地编辑**（与 clothes 那段的 `v-model="item.name"` 同款）：父组件
// 的 ref 是深响应式的，嵌套变更会同步回去；不需要回传事件，提交由父组件把握。
const draft = computed(() => props.modelValue);

const { t } = useI18n();
const uiStore = useUIStore();

const addHotword = () => {
  props.modelValue.hotwords.push({
    text: "",
    weight: HOTWORD_DEFAULT_WEIGHT,
    role_id: props.roleId,
  });
};

const removeHotword = (idx: number) => {
  props.modelValue.hotwords.splice(idx, 1);
};

/** 批量删除模式：开启后每行显示复选框，删除只作用于草稿，仍由宿主的「保存」落库。 */
const batchMode = ref(false);
const selected = reactive(new Set<RoleHotword>());

const selectedCount = computed(() =>
  draft.value.hotwords.reduce((count, row) => (selected.has(row) ? count + 1 : count), 0),
);
const allSelected = computed(
  () => draft.value.hotwords.length > 0 && draft.value.hotwords.every((row) => selected.has(row)),
);

const toggleRow = (row: RoleHotword) => {
  if (selected.has(row)) selected.delete(row);
  else selected.add(row);
};

const toggleAll = () => {
  if (allSelected.value) {
    for (const row of draft.value.hotwords) selected.delete(row);
  } else {
    for (const row of draft.value.hotwords) selected.add(row);
  }
};

const toggleBatchMode = () => {
  batchMode.value = !batchMode.value;
  if (!batchMode.value) selected.clear();
};

const deleteSelected = () => {
  if (selectedCount.value === 0) return;
  const rows = draft.value.hotwords;
  rows.splice(0, rows.length, ...rows.filter((row) => !selected.has(row)));
  selected.clear();
  batchMode.value = false;
};

const commitText = (item: RoleHotword) => commitRowText(props.modelValue.hotwords, item);
const clampWeight = (item: RoleHotword) => clampRowWeight(item);

/** 档位标签：50 是「超级热词」（厂商口径），其余显示数字。 */
const weightLabel = (w: number) =>
  w === 50 ? t("settings.characterInfo.hotword.weight.super") : String(w);

const showPicker = ref(false);
const showImport = ref(false);

/** 从词库添加：追加到列表尾部，`role_id` 沿用宿主给的值（角色侧是真实 roleId，
 *  全局侧是 0 占位——绝不能在角色侧用 0，那是 User 的真实 role_id）。 */
const appendFromLibrary = (words: LibraryWord[]) => {
  for (const word of words) {
    props.modelValue.hotwords.push({
      text: word.text,
      weight: word.weight,
      role_id: props.roleId,
    });
  }
  // 并入后可能与草稿里已有的撞键（选择器挡的是**打开那一刻**的状态，词条期间可能被改）。
  // 复用共享的整表去重（泛型签名同时吃 `RoleHotword` 与 `LibraryWord`）。
  dedupeRows(props.modelValue.hotwords);
};

/** 批量导入：先在副本上追加并去重，保存成功后才写回草稿；失败时宿主草稿不变。 */
const handleImportSave = async (rows: ImportRow[]) => {
  const next = props.modelValue.hotwords.map((row) => ({ ...row }));
  for (const row of rows) {
    next.push({ text: row.text, weight: row.weight, role_id: props.roleId });
  }
  dedupeRows(next);

  await props.saveHotwords(next);
  props.modelValue.hotwords.splice(0, props.modelValue.hotwords.length, ...next);

  uiStore.showNotification({
    type: "success",
    title: t("settings.hotwordImport.notification.title", { count: rows.length }),
    message: t(
      props.scope === "global"
        ? "settings.hotwordImport.notification.scopeGlobal"
        : "settings.hotwordImport.notification.scopeRole",
    ),
    skipTipsCheck: true,
  });
};
</script>

<style scoped>
/* 与弹窗内联表单同款聚焦态（父组件的 .form-control 是 scoped 的，子组件拿不到） */
.hotword-control:focus {
  border-color: #79d9ff;
  background: rgba(0, 0, 0, 0.3);
  box-shadow: 0 0 0 3px rgba(121, 217, 255, 0.2);
}
</style>

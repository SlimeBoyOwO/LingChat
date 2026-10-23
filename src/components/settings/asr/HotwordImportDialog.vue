<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="visible"
        class="fixed inset-0 z-[10001] flex items-center justify-center bg-black/40 p-2 backdrop-blur-sm sm:p-4"
        @click="handleClose"
      >
        <div
          class="flex h-[95dvh] w-full max-w-[960px] flex-col overflow-hidden rounded-3xl border border-white/20 bg-[linear-gradient(135deg,rgba(255,255,255,0.15)_0%,rgba(255,255,255,0.05)_100%)] text-white shadow-[0_20px_60px_rgba(0,0,0,0.4),inset_0_0_1px_rgba(255,255,255,0.3)] backdrop-blur-[30px] backdrop-saturate-180 sm:h-[85dvh]"
          @click.stop
        >
          <!-- Header -->
          <div
            class="flex shrink-0 items-center justify-between border-b border-white/10 bg-[linear-gradient(180deg,rgba(255,255,255,0.1)_0%,rgba(255,255,255,0.05)_100%)] p-4 sm:p-6"
          >
            <div class="flex min-w-0 items-center gap-3 sm:gap-4">
              <div
                class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-white/10 shadow-inner sm:h-12 sm:w-12"
              >
                <svg
                  class="h-5 w-5 sm:h-6 sm:w-6"
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
                  <path d="M12 3v12" />
                  <path d="m7 10 5 5 5-5" />
                  <path d="M5 21h14" />
                </svg>
              </div>
              <div class="min-w-0">
                <h2
                  class="m-0 text-lg font-bold drop-shadow-[0_2px_4px_rgba(0,0,0,0.3)] sm:text-xl"
                >
                  {{ $t("settings.hotwordImport.dialog.title") }}
                </h2>
                <p class="m-0 mt-0.5 text-xs text-white/50 sm:text-sm">
                  {{ $t("settings.hotwordImport.dialog.subtitle") }}
                </p>
              </div>
            </div>
            <button
              class="flex h-9 w-9 shrink-0 cursor-pointer items-center justify-center rounded-full border-none bg-white/10 text-white transition-all duration-200 hover:rotate-90 hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="saving"
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

          <!-- Body -->
          <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4 sm:p-6">
            <textarea
              v-model="text"
              class="min-h-[140px] flex-1 resize-none rounded-xl border border-white/10 bg-black/20 p-3 font-sans text-[13px] leading-relaxed text-white transition-all duration-200 outline-none placeholder:text-white/25 focus:border-[#79d9ff]/60 focus:bg-black/30 focus:ring-2 focus:ring-[#79d9ff]/10 disabled:cursor-not-allowed disabled:opacity-60"
              :placeholder="$t('settings.hotwordImport.dialog.placeholder')"
              :disabled="saving"
              spellcheck="false"
              autocomplete="off"
            ></textarea>

            <!-- 状态行：左侧只显示异常数；超限另起一段 -->
            <div
              class="flex min-h-7 shrink-0 flex-wrap items-center gap-x-3 gap-y-1 text-xs sm:text-[13px]"
            >
              <span :class="review.invalidCount > 0 ? 'text-red-300' : 'text-white/40'">
                {{
                  review.invalidCount > 0
                    ? $t("settings.hotwordImport.dialog.invalidCount", {
                        count: review.invalidCount,
                      })
                    : $t("settings.hotwordImport.dialog.noInvalid")
                }}
              </span>
              <span v-if="review.overCapacityCount > 0" class="text-amber-300">
                {{
                  $t("settings.hotwordImport.dialog.overCapacityCount", {
                    count: review.overCapacityCount,
                  })
                }}
              </span>
              <span v-if="listOverLimit" class="text-amber-300">
                {{ $t("settings.hotwordImport.dialog.listOverLimit") }}
              </span>
            </div>

            <!-- 保存被拒后才展开的异常明细 -->
            <div
              v-if="showDetails && review.details.length > 0"
              class="max-h-44 shrink-0 space-y-1 overflow-y-auto p-1"
            >
              <div
                v-for="(detail, index) in visibleDetails"
                :key="`${detail.kind}-${detail.line}-${detail.reason}-${index}`"
                class="rounded-lg border border-red-400/20 bg-red-500/10 px-2.5 py-1.5 text-xs"
              >
                <div class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
                  <span class="shrink-0 text-red-200/70">
                    {{ $t("settings.hotwordImport.dialog.line", { line: detail.line }) }}
                  </span>
                  <span class="min-w-0 flex-1 break-all text-white/80">{{ detail.text }}</span>
                  <span v-if="detailWeightValue(detail) !== ''" class="shrink-0 text-white/40">
                    {{ detailWeightText(detail) }}
                  </span>
                </div>
                <p class="m-0 mt-0.5 text-red-300">{{ detailReason(detail) }}</p>
              </div>
              <p v-if="extraDetailCount > 0" class="px-1 text-[11px] text-white/40">
                {{ $t("settings.hotwordImport.dialog.more", { count: extraDetailCount }) }}
              </p>
            </div>

            <p v-if="saveError" class="m-0 shrink-0 text-xs text-red-300">{{ saveError }}</p>

            <!-- 字段行：角色/全局没有目标分类，整行隐藏 -->
            <div v-if="showCategory" class="flex shrink-0 flex-wrap items-end gap-3">
              <label class="min-w-0 flex-1">
                <span class="mb-1 flex items-center gap-1.5 text-xs text-white/55">
                  {{ $t("settings.hotwordImport.dialog.targetCategory") }}
                  <span class="rounded bg-white/10 px-1.5 py-0.5 text-[10px] text-white/40">
                    {{ $t("settings.hotwordImport.dialog.libraryOnly") }}
                  </span>
                </span>
                <select
                  v-model="categoryValue"
                  class="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white transition-all duration-200 outline-none focus:border-[#79d9ff]/60 focus:ring-2 focus:ring-[#79d9ff]/10 disabled:cursor-not-allowed disabled:opacity-60"
                  :disabled="saving"
                >
                  <option
                    v-for="opt in categoryOptions"
                    :key="opt.value"
                    :value="opt.value"
                    class="bg-[#333] text-white"
                  >
                    {{ opt.label }}
                  </option>
                </select>
              </label>
            </div>

            <!-- 说明文案：词库看重复词分类规则，角色/全局看保存范围 -->
            <div class="shrink-0 space-y-1 text-xs leading-relaxed text-white/40">
              <p class="m-0">{{ $t("settings.hotwordImport.dialog.appendHint") }}</p>
              <p class="m-0">
                {{
                  showCategory
                    ? $t("settings.hotwordImport.dialog.duplicateCategoryHint")
                    : $t("settings.hotwordImport.dialog.saveScopeHint")
                }}
              </p>
            </div>
          </div>

          <!-- Footer -->
          <div
            class="flex shrink-0 justify-end gap-3 border-t border-white/10 bg-[linear-gradient(180deg,rgba(255,255,255,0.05)_0%,rgba(255,255,255,0.1)_100%)] p-3 sm:p-4"
          >
            <button
              class="cursor-pointer rounded-[20px] border-none bg-white/10 px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-50"
              :disabled="saving"
              @click="handleClose"
            >
              {{ $t("settings.hotwordImport.dialog.cancel") }}
            </button>
            <button
              class="cursor-pointer rounded-[20px] border-none bg-[#79d9ff] px-5 py-2 text-sm font-medium text-[#10242e] transition-all duration-200 hover:enabled:-translate-y-px hover:enabled:bg-[#9ce5ff] hover:enabled:shadow-[0_4px_12px_rgba(121,217,255,0.3)] disabled:cursor-not-allowed disabled:opacity-50"
              :disabled="saving || text.trim() === ''"
              @click="handleSave"
            >
              <span
                v-if="saving"
                class="mr-2 inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-[#10242e]/30 border-t-[#10242e]"
              ></span>
              <span v-else class="mr-1">✓</span>
              {{
                saving
                  ? $t("settings.hotwordImport.dialog.saving")
                  : $t("settings.hotwordImport.dialog.save")
              }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import {
  MAX_HOTWORD_CHARS,
  MAX_HOTWORD_SEGMENTS,
  hotwordCharCount,
  hotwordSegmentCount,
} from "@/api/services/hotword";
import { CATEGORY_NONE } from "@/utils/hotwordLibrary";
import { parseHotwordImport, type ImportDetail, type ImportRow } from "@/utils/hotwordImport";

const props = defineProps<{
  visible: boolean;
  existing: { text: string; category_index?: number | null }[];
  capacity: number;
  /** 仅词库上下文传入；角色/全局不传，目标分类控件隐藏。 */
  categories?: string[];
  defaultCategoryIndex?: number | null;
  /** 由宿主实现：追加到草稿并保存；抛错时面板保持打开并显示错误。 */
  onSave: (rows: ImportRow[], categoryIndex: number | null) => Promise<void>;
}>();

const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();

const text = ref("");
const categoryIndex = ref<number | null>(null);
const showDetails = ref(false);
const saving = ref(false);
const saveError = ref("");

const review = computed(() =>
  parseHotwordImport(
    text.value,
    props.existing,
    props.capacity,
    props.categories === undefined ? undefined : categoryIndex.value,
  ),
);

/** 当前草稿本身已超容量：连只导入重复词也不允许保存。 */
const listOverLimit = computed(() => props.existing.length > props.capacity);

const showCategory = computed(() => props.categories !== undefined);

const categoryOptions = computed(() => [
  {
    value: CATEGORY_NONE,
    label: t("settings.hotwordImport.dialog.uncategorized"),
  },
  ...(props.categories ?? []).map((name, index) => ({
    value: String(index),
    label: name.trim() === "" ? t("settings.hotwordImport.dialog.unnamed") : name,
  })),
]);

const categoryValue = computed({
  get: () => (categoryIndex.value === null ? CATEGORY_NONE : String(categoryIndex.value)),
  set: (value: string) => {
    categoryIndex.value = value === CATEGORY_NONE ? null : Number(value);
  },
});

const visibleDetails = computed(() => review.value.details.slice(0, 20));
const extraDetailCount = computed(() => Math.max(0, review.value.details.length - 20));

const detailWeightValue = (detail: ImportDetail) => String(detail.weight).trim();

const detailWeightText = (detail: ImportDetail) =>
  `${t("settings.hotwordImport.dialog.weight")} ${detailWeightValue(detail)}`;

const detailReason = (detail: ImportDetail) => {
  switch (detail.reason) {
    case "badFormat":
      return t("settings.hotwordImport.dialog.reason.badFormat");
    case "tooLongChars":
      return t("settings.hotwordImport.dialog.reason.tooLongChars", {
        count: hotwordCharCount(detail.text),
        max: MAX_HOTWORD_CHARS,
      });
    case "tooLongSegments":
      return t("settings.hotwordImport.dialog.reason.tooLongSegments", {
        count: hotwordSegmentCount(detail.text),
        max: MAX_HOTWORD_SEGMENTS,
      });
    case "badWeight":
      return t("settings.hotwordImport.dialog.reason.badWeight", {
        weight: detailWeightValue(detail),
      });
    case "overCapacity":
      return t("settings.hotwordImport.dialog.reason.overCapacity");
    default:
      return "";
  }
};

const normalizeDefaultCategory = () => {
  const categories = props.categories;
  const index = props.defaultCategoryIndex;
  if (!categories || index == null) return null;
  return index >= 0 && index < categories.length ? index : null;
};

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    showDetails.value = false;
    saveError.value = "";
    categoryIndex.value = normalizeDefaultCategory();
  },
  { immediate: true },
);

watch(text, () => {
  if (saveError.value) saveError.value = "";
});

const handleClose = () => {
  if (saving.value) return;
  emit("close");
};

const handleSave = async () => {
  if (saving.value) return;

  // 当前草稿已超容量时直接拒绝，避免导入后再触发整表超限
  if (listOverLimit.value) {
    saveError.value = t("settings.hotwordImport.dialog.listOverLimit");
    return;
  }

  if (review.value.invalidCount > 0 || review.value.overCapacityCount > 0) {
    showDetails.value = true;
    return;
  }
  if (review.value.importableCount === 0) {
    saveError.value = t("settings.hotwordImport.dialog.noRows");
    return;
  }

  saving.value = true;
  saveError.value = "";
  try {
    await props.onSave(review.value.rows, categoryIndex.value);
    text.value = "";
    showDetails.value = false;
    emit("close");
  } catch (error) {
    saveError.value = error instanceof Error ? error.message : String(error);
  } finally {
    saving.value = false;
  }
};
</script>

<style scoped>
.modal-enter-active,
.modal-leave-active {
  transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
  transform: scale(0.95) translateY(10px);
}
</style>

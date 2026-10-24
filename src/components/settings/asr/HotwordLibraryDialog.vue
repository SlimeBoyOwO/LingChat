<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="visible"
        class="fixed inset-0 z-[10000] flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm"
        @click="handleClose"
      >
        <div
          class="flex h-[85dvh] w-full max-w-3xl flex-col overflow-hidden rounded-3xl border border-white/20 bg-[linear-gradient(135deg,rgba(255,255,255,0.15)_0%,rgba(255,255,255,0.05)_100%)] text-white shadow-[0_20px_60px_rgba(0,0,0,0.4),inset_0_0_1px_rgba(255,255,255,0.3)] backdrop-blur-[30px] backdrop-saturate-180"
          @click.stop
        >
          <div
            class="flex items-center justify-between border-b border-white/10 bg-[linear-gradient(180deg,rgba(255,255,255,0.1)_0%,rgba(255,255,255,0.05)_100%)] p-6"
          >
            <div class="flex items-center gap-4">
              <div
                class="flex h-12 w-12 items-center justify-center rounded-xl bg-white/10 shadow-inner"
              >
                <Icon icon="mic" />
              </div>
              <div>
                <h2 class="m-0 text-xl font-bold drop-shadow-[0_2px_4px_rgba(0,0,0,0.3)]">
                  {{ $t("settings.asr.hotwordLibrary.dialog.title") }}
                </h2>
                <p class="m-0 text-sm text-white/50">
                  {{ $t("settings.asr.hotwordLibrary.dialog.subtitle") }}
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

          <div class="relative flex-1 overflow-y-auto p-6">
            <div v-if="loading" class="flex h-full items-center justify-center">
              <div
                class="h-10 w-10 animate-spin rounded-full border-3 border-white/10 border-t-[#5e72e4]"
              ></div>
            </div>

            <div v-else class="flex h-full min-h-0 gap-4">
              <aside
                class="flex w-56 shrink-0 flex-col rounded-2xl border border-white/10 bg-black/20"
              >
                <ul class="flex-1 overflow-y-auto p-2">
                  <li>
                    <button
                      class="w-full cursor-pointer rounded-lg border-none bg-transparent px-3 py-2 text-left text-sm"
                      :class="selected === 'all' ? 'bg-white/15 text-white' : 'text-white/70'"
                      @click="selected = 'all'"
                    >
                      {{ $t("settings.asr.hotwordLibrary.dialog.categories.all") }}
                      <span class="float-right text-white/40">{{ countWords(draft, "all") }}</span>
                    </button>
                  </li>
                  <li>
                    <button
                      class="w-full cursor-pointer rounded-lg border-none bg-transparent px-3 py-2 text-left text-sm"
                      :class="selected === 'none' ? 'bg-white/15 text-white' : 'text-white/70'"
                      @click="selected = 'none'"
                    >
                      {{ $t("settings.asr.hotwordLibrary.dialog.categories.uncategorized") }}
                      <span class="float-right text-white/40">
                        {{ countWords(draft, "none") }}
                      </span>
                    </button>
                  </li>
                  <li
                    v-for="(name, index) in draft.categories"
                    :key="index"
                    class="group flex items-center"
                  >
                    <input
                      :id="`hotword-category-${index}`"
                      v-model="draft.categories[index]"
                      type="text"
                      class="hotword-control min-w-0 flex-1 rounded-lg border bg-transparent px-3 py-2 text-sm text-white"
                      :class="
                        invalidCategory === index ? 'border-red-400/60' : 'border-transparent'
                      "
                      :placeholder="
                        $t('settings.asr.hotwordLibrary.dialog.categories.namePlaceholder')
                      "
                      @click="selected = index"
                      @change="commitCategoryName(index)"
                    />
                    <span class="w-6 shrink-0 text-right text-xs text-white/40">
                      {{ countWords(draft, index) }}
                    </span>
                    <button
                      :title="$t('settings.asr.hotwordLibrary.dialog.categories.delete')"
                      class="h-5 w-5 shrink-0 cursor-pointer rounded-full border-none bg-red-500/20 text-[10px] text-red-400 opacity-0 group-hover:opacity-100"
                      @click="deleteCategory(index)"
                    >
                      ×
                    </button>
                  </li>
                </ul>
                <!-- 与词条数上限同一套提示样式；超限时后端整表拒绝，保存只会回一句 saveFailed -->
                <p
                  v-if="tooManyCategories"
                  class="mx-2 mb-2 rounded-xl border border-red-400/30 bg-red-500/10 px-3 py-2 text-[12px] text-red-200"
                >
                  {{
                    $t("settings.asr.hotwordLibrary.dialog.validation.tooManyCategories", {
                      count: draft.categories.length,
                      max: MAX_LIBRARY_CATEGORIES,
                    })
                  }}
                </p>
                <button
                  class="m-2 cursor-pointer rounded-lg border border-dashed border-white/20 bg-transparent px-3 py-2 text-xs text-white/60 hover:border-[#5e72e4]/50 hover:text-white"
                  @click="addCategory"
                >
                  ＋ {{ $t("settings.asr.hotwordLibrary.dialog.categories.new") }}
                </button>
              </aside>

              <div class="flex min-w-0 flex-1 flex-col">
                <div class="mb-3 flex items-center gap-2">
                  <input
                    v-model="search"
                    type="text"
                    class="hotword-control min-w-0 flex-1 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white"
                    :placeholder="$t('settings.asr.hotwordLibrary.dialog.words.searchPlaceholder')"
                  />
                  <button
                    class="shrink-0 cursor-pointer rounded-lg border-none bg-[#5e72e4] px-3 py-2 text-xs text-white hover:bg-[#4a5acf]"
                    @click="addWord"
                  >
                    ＋ {{ $t("settings.asr.hotwordLibrary.dialog.words.add") }}
                  </button>
                </div>

                <p
                  v-if="tooManyWords"
                  class="mb-2 rounded-xl border border-red-400/30 bg-red-500/10 px-3 py-2 text-[12px] text-red-200"
                >
                  {{
                    $t("settings.asr.hotwordLibrary.dialog.validation.tooManyWords", {
                      count: draft.words.length,
                      max: MAX_LIBRARY_ROWS,
                    })
                  }}
                </p>

                <div
                  v-if="visibleWords.length > 0"
                  class="mb-1 flex items-center gap-2 text-[13px] text-white/40"
                >
                  <span class="flex-1">{{ $t("settings.characterInfo.hotword.colText") }}</span>
                  <span class="w-32 shrink-0">
                    {{ $t("settings.asr.hotwordLibrary.dialog.words.colCategory") }}
                  </span>
                  <span class="w-24 shrink-0">
                    {{ $t("settings.characterInfo.hotword.colWeight") }}
                  </span>
                  <span class="w-6 shrink-0"></span>
                </div>

                <div class="min-h-0 flex-1 overflow-y-auto">
                  <div v-for="word in visibleWords" :key="draft.words.indexOf(word)" class="mb-1">
                    <div class="flex items-center gap-2">
                      <input
                        v-model="word.text"
                        type="text"
                        class="hotword-control min-w-0 flex-1 rounded-xl border bg-black/20 px-3 py-2 text-sm text-white"
                        :class="
                          isHotwordTextOverlong(word.text) ? 'border-red-400/60' : 'border-white/10'
                        "
                        @change="commitWordText(word)"
                      />
                      <select
                        :value="indexToFilter(word.category_index)"
                        class="hotword-control w-32 shrink-0 rounded-xl border border-white/10 bg-black/20 px-2 py-2 text-sm text-white"
                        @change="
                          word.category_index = filterToIndex(
                            ($event.target as HTMLSelectElement).value,
                          )
                        "
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
                      <select
                        v-model.number="word.weight"
                        class="hotword-control w-24 shrink-0 rounded-xl border border-white/10 bg-black/20 px-2 py-2 text-sm text-white"
                        @change="clampWordWeight(word)"
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
                        class="h-6 w-6 shrink-0 cursor-pointer rounded-full border-none bg-red-500/20 text-xs text-red-400 hover:bg-red-500/40"
                        @click="removeWord(word)"
                      >
                        ×
                      </button>
                    </div>
                    <!-- 超长条目会被后端整表拒；提示按字符数 / 片段数两套口径走。 -->
                    <p
                      v-if="hotwordLimitKind(word.text) !== 'ok'"
                      class="mt-1 text-[12px] text-red-300"
                    >
                      {{
                        hotwordLimitKind(word.text) === "segments"
                          ? $t("settings.characterInfo.hotword.tooLongSegments", {
                              count: hotwordSegmentCount(word.text),
                              max: MAX_HOTWORD_SEGMENTS,
                            })
                          : $t("settings.characterInfo.hotword.tooLongChars", {
                              count: hotwordCharCount(word.text),
                              max: MAX_HOTWORD_CHARS,
                            })
                      }}
                    </p>
                  </div>

                  <div
                    v-if="visibleWords.length === 0"
                    class="py-8 text-center text-sm text-white/40"
                  >
                    {{ $t("settings.asr.hotwordLibrary.dialog.words.empty") }}
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div
            class="flex justify-end gap-3 border-t border-white/10 bg-[linear-gradient(180deg,rgba(255,255,255,0.05)_0%,rgba(255,255,255,0.1)_100%)] p-4"
          >
            <button
              class="cursor-pointer rounded-[20px] border-none bg-white/10 px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:bg-white/20"
              @click="handleClose"
            >
              {{ $t("settings.asr.hotwordLibrary.dialog.footer.cancel") }}
            </button>
            <button
              class="cursor-pointer rounded-[20px] border-none bg-[#5e72e4] px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:enabled:-translate-y-px hover:enabled:bg-[#4a5acf] hover:enabled:shadow-[0_4px_12px_rgba(94,114,228,0.3)] disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="saving || loading"
              @click="save"
            >
              <span
                v-if="saving"
                class="mr-2 inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/30 border-t-white"
              ></span>
              {{
                saving
                  ? $t("settings.asr.hotwordLibrary.dialog.footer.saving")
                  : $t("settings.asr.hotwordLibrary.dialog.footer.save")
              }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import {
  MAX_HOTWORD_CHARS,
  MAX_HOTWORD_SEGMENTS,
  MAX_LIBRARY_CATEGORIES,
  MAX_LIBRARY_ROWS,
  getHotwordLibrary,
  hotwordCharCount,
  hotwordLimitKind,
  hotwordSegmentCount,
  isHotwordTextOverlong,
  saveHotwordLibrary,
  type LibraryWord,
} from "@/api/services/hotword";
import { Icon } from "@/components/base";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import {
  CATEGORY_NONE,
  countWords,
  filterToIndex,
  indexToFilter,
  isCategoryNameValid,
  removeCategory,
  type CategoryFilter,
  type LibraryDraft,
} from "@/utils/hotwordLibrary";
import {
  HOTWORD_DEFAULT_WEIGHT,
  HOTWORD_WEIGHT_OPTIONS,
  clampRowWeight,
  commitRowText,
} from "@/utils/hotwordRows";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const dialogStore = useDialogStore();

const loading = ref(false);
const saving = ref(false);
const draft = ref<LibraryDraft>({ categories: [], words: [] });
/** 左栏选中项。`"all"` = 全部（默认），`"none"` = 未分类，数字 = 分类下标。 */
const selected = ref<CategoryFilter>("all");
const search = ref("");
/** 保存前校验失败时高亮的分类下标（就地标红并定位）。 */
const invalidCategory = ref<number | null>(null);

watch(
  () => props.visible,
  async (visible) => {
    if (!visible) return;
    loading.value = true;
    invalidCategory.value = null;
    selected.value = "all";
    search.value = "";
    try {
      // 保存是整表替换，读失败必须让弹窗打不开——草稿默认是空表，若放行，
      // 用户随手点一次「保存」就会静默清空整个词库
      draft.value = await getHotwordLibrary();
    } catch (e) {
      console.error("[HotwordLibraryDialog] 读取热词库失败:", e);
      await dialogStore.alert(t("settings.asr.hotwordLibrary.dialog.loadFailed"));
      emit("close");
    } finally {
      loading.value = false;
    }
  },
);

/** 右侧词条表：按左栏选中 + 搜索过滤（行内编辑作用于原数组元素，故持有引用）。 */
const visibleWords = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  return draft.value.words.filter((w) => {
    const byCategory =
      selected.value === "all"
        ? true
        : w.category_index === (selected.value === "none" ? null : selected.value);
    const bySearch = keyword === "" || w.text.toLowerCase().includes(keyword);
    return byCategory && bySearch;
  });
});

const tooManyWords = computed(() => draft.value.words.length > MAX_LIBRARY_ROWS);
const tooManyCategories = computed(() => draft.value.categories.length > MAX_LIBRARY_CATEGORIES);

/** 分类下拉的选项：未分类 + 全部分类（值用 `CATEGORY_NONE` / 下标字符串）。 */
const categoryOptions = computed(() => [
  { value: CATEGORY_NONE, label: t("settings.asr.hotwordLibrary.dialog.categories.uncategorized") },
  ...draft.value.categories.map((name, index) => ({
    value: String(index),
    label: name.trim() === "" ? t("settings.asr.hotwordLibrary.dialog.categories.unnamed") : name,
  })),
]);

/** 新建分类：追加空白名行并聚焦输入（末尾追加 ⇒ 已有下标不动，天然安全）。 */
const addCategory = async () => {
  draft.value.categories.push("");
  const index = draft.value.categories.length - 1;
  selected.value = index;
  await nextTick();
  document.getElementById(`hotword-category-${index}`)?.focus();
};

const deleteCategory = async (index: number) => {
  const count = countWords(draft.value, index);
  const confirmed = await dialogStore.confirm(
    t("settings.asr.hotwordLibrary.dialog.categories.deleteConfirm", { count }),
  );
  if (!confirmed) return;
  // 重排 + 级联删除 + 再次去重全在纯函数里
  removeCategory(draft.value, index);
  selected.value = "all";
  invalidCategory.value = null;
};

/** 行内重命名：失焦时 trim（与 DB 的 UNIQUE(name) BINARY 口径一致）。 */
const commitCategoryName = (index: number) => {
  draft.value.categories[index] = draft.value.categories[index].trim();
  invalidCategory.value = null;
};

const addWord = () => {
  // 清空搜索：新行是空词条，留着关键词会被过滤掉，用户以为按钮没反应
  search.value = "";
  draft.value.words.push({
    // 选中「全部」时默认未分类
    category_index: typeof selected.value === "number" ? selected.value : null,
    text: "",
    weight: HOTWORD_DEFAULT_WEIGHT,
  });
};

const removeWord = (word: LibraryWord) => {
  const index = draft.value.words.indexOf(word);
  if (index !== -1) draft.value.words.splice(index, 1);
};

const commitWordText = (word: LibraryWord) => commitRowText(draft.value.words, word);
const clampWordWeight = (word: LibraryWord) => clampRowWeight(word);

/** 档位标签：50 是「超级热词」（厂商口径），其余显示数字。 */
const weightLabel = (w: number) =>
  w === 50 ? t("settings.characterInfo.hotword.weight.super") : String(w);

/** 分类名各问题对应的提示文案；未知原因兜底 `checkForm`。 */
const categoryNameErrorKeys: Record<string, string> = {
  empty: "settings.asr.hotwordLibrary.dialog.validation.emptyName",
  tooLong: "settings.asr.hotwordLibrary.dialog.validation.nameTooLong",
  duplicate: "settings.asr.hotwordLibrary.dialog.validation.duplicateName",
};

const save = async () => {
  // 前端先拦分类名问题：后端也会拒，但那是整个库回滚 + 一句「保存失败」，
  // 用户定位不到是哪一行
  for (let i = 0; i < draft.value.categories.length; i += 1) {
    const reason = isCategoryNameValid(draft.value, i);
    if (reason === "ok") continue;
    invalidCategory.value = i;
    const key =
      categoryNameErrorKeys[reason] ?? "settings.asr.hotwordLibrary.dialog.validation.checkForm";
    await dialogStore.alert(t(key));
    return;
  }

  saving.value = true;
  try {
    await saveHotwordLibrary(draft.value.categories, draft.value.words);
    // 无需刷新任何缓存：选择器每次打开现读，识别链路不读库
    emit("close");
  } catch (e) {
    console.error("[HotwordLibraryDialog] 保存热词库失败:", e);
    await dialogStore.alert(t("settings.asr.hotwordLibrary.dialog.saveFailed"));
  } finally {
    saving.value = false;
  }
};

const handleClose = () => emit("close");
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

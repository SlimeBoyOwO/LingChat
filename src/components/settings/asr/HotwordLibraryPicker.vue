<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="visible"
        class="fixed inset-0 z-[10000] flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm"
        @click="handleClose"
      >
        <div
          class="flex h-[70dvh] w-full max-w-2xl flex-col overflow-hidden rounded-3xl border border-white/20 bg-[linear-gradient(135deg,rgba(255,255,255,0.15)_0%,rgba(255,255,255,0.05)_100%)] text-white shadow-[0_20px_60px_rgba(0,0,0,0.4),inset_0_0_1px_rgba(255,255,255,0.3)] backdrop-blur-[30px] backdrop-saturate-180"
          @click.stop
        >
          <!-- Header：与 GlobalHotwordDialog 同款外壳，仅换文案 -->
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
                  {{ $t("settings.asr.hotwordLibrary.picker.title") }}
                </h2>
                <p class="m-0 text-sm text-white/50">
                  {{ $t("settings.asr.hotwordLibrary.picker.selected", { count: picked.size }) }}
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
            <div v-else class="flex h-full flex-col">
              <div class="mb-3 flex items-center gap-2">
                <select
                  v-model="categoryFilter"
                  class="hotword-control w-40 shrink-0 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white"
                >
                  <option value="all" class="bg-[#333]">
                    {{ $t("settings.asr.hotwordLibrary.picker.allCategories") }}
                  </option>
                  <option :value="CATEGORY_NONE" class="bg-[#333]">
                    {{ $t("settings.asr.hotwordLibrary.picker.uncategorized") }}
                  </option>
                  <option
                    v-for="(name, index) in categories"
                    :key="index"
                    :value="String(index)"
                    class="bg-[#333]"
                  >
                    {{ name }}
                  </option>
                </select>
                <input
                  v-model="search"
                  type="text"
                  class="hotword-control min-w-0 flex-1 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white"
                  :placeholder="$t('settings.asr.hotwordLibrary.picker.searchPlaceholder')"
                />
                <button
                  class="shrink-0 cursor-pointer rounded-lg border-none bg-white/10 px-3 py-2 text-xs text-white hover:bg-white/20"
                  @click="toggleAll"
                >
                  {{
                    allSelected
                      ? $t("settings.asr.hotwordLibrary.picker.clear")
                      : $t("settings.asr.hotwordLibrary.picker.selectAll")
                  }}
                </button>
              </div>

              <div class="min-h-0 flex-1 overflow-y-auto">
                <div
                  v-for="(row, index) in filtered"
                  :key="index"
                  class="flex items-center gap-3 rounded-xl px-3 py-2"
                  :class="row.existing ? 'opacity-50' : 'hover:bg-white/5'"
                >
                  <input
                    type="checkbox"
                    class="shrink-0"
                    :disabled="row.existing"
                    :checked="picked.has(row.word)"
                    @change="toggle(row.word)"
                  />
                  <span class="min-w-0 flex-1 truncate text-sm">{{ row.word.text }}</span>
                  <span class="w-8 shrink-0 text-right text-xs text-white/50">
                    {{ row.word.weight }}
                  </span>
                  <span v-if="row.existing" class="shrink-0 text-xs text-amber-300">
                    {{ $t("settings.asr.hotwordLibrary.picker.existing") }}
                  </span>
                </div>

                <div v-if="filtered.length === 0" class="py-8 text-center text-sm text-white/40">
                  {{ $t("settings.asr.hotwordLibrary.picker.empty") }}
                </div>
              </div>
            </div>
          </div>

          <!-- Footer -->
          <div
            class="flex items-center justify-between gap-3 border-t border-white/10 bg-[linear-gradient(180deg,rgba(255,255,255,0.05)_0%,rgba(255,255,255,0.1)_100%)] p-4"
          >
            <span class="text-sm text-white/60">
              {{ $t("settings.asr.hotwordLibrary.picker.selected", { count: picked.size }) }}
            </span>
            <div class="flex gap-3">
              <button
                class="cursor-pointer rounded-[20px] border-none bg-white/10 px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:bg-white/20"
                @click="handleClose"
              >
                {{ $t("settings.asr.hotwordLibrary.picker.cancel") }}
              </button>
              <button
                class="cursor-pointer rounded-[20px] border-none bg-[#5e72e4] px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:enabled:-translate-y-px hover:enabled:bg-[#4a5acf] disabled:cursor-not-allowed disabled:opacity-60"
                :disabled="picked.size === 0 || loading"
                @click="confirm"
              >
                {{ $t("settings.asr.hotwordLibrary.picker.add") }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import { getHotwordLibrary, type LibraryWord, type RoleHotword } from "@/api/services/hotword";
import { Icon } from "@/components/base";
import { CATEGORY_NONE } from "@/utils/hotwordLibrary";
import { useDialogStore } from "@/stores/modules/ui/dialog";

const props = defineProps<{
  visible: boolean;
  /** 宿主当前草稿里的词条：与其 trim 后 text 相同的库词条标「已存在」且不可勾选。 */
  existing: RoleHotword[];
}>();
const emit = defineEmits<{ close: []; add: [words: LibraryWord[]] }>();

const { t } = useI18n();
const dialogStore = useDialogStore();

const loading = ref(false);
const categories = ref<string[]>([]);
const words = ref<LibraryWord[]>([]);
const search = ref("");
const categoryFilter = ref<string>("all");
const picked = ref(new Set<LibraryWord>());

/** 防重键与共享的 `dedupeRows` / DB 的 UNIQUE 同口径：trim 后的原样字符串。 */
const existingKeys = computed(() => new Set(props.existing.map((w) => w.text.trim())));

const filtered = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  return words.value
    .map((word) => ({
      word,
      existing: existingKeys.value.has(word.text.trim()),
    }))
    .filter((row) => {
      const byCategory =
        categoryFilter.value === "all"
          ? true
          : row.word.category_index ===
            (categoryFilter.value === CATEGORY_NONE ? null : Number(categoryFilter.value));
      const bySearch = keyword === "" || row.word.text.toLowerCase().includes(keyword);
      return byCategory && bySearch;
    });
});

const selectable = computed(() => filtered.value.filter((row) => !row.existing));
const allSelected = computed(
  () => selectable.value.length > 0 && selectable.value.every((row) => picked.value.has(row.word)),
);

const toggle = (word: LibraryWord) => {
  const next = new Set(picked.value);
  if (next.has(word)) next.delete(word);
  else next.add(word);
  picked.value = next;
};

/** 全选 / 清空：只作用于**当前过滤结果**里可勾选的那些。 */
const toggleAll = () => {
  const next = new Set(picked.value);
  for (const row of selectable.value) {
    if (allSelected.value) next.delete(row.word);
    else next.add(row.word);
  }
  picked.value = next;
};

watch(
  () => props.visible,
  async (visible) => {
    if (!visible) return;
    loading.value = true;
    search.value = "";
    categoryFilter.value = "all";
    picked.value = new Set();
    try {
      const lib = await getHotwordLibrary();
      categories.value = lib.categories;
      words.value = lib.words;
    } catch (e) {
      console.error("[HotwordLibraryPicker] 读取热词库失败:", e);
      // 先关选择器再弹提示：选择器是自带全屏遮罩的 fixed 层，留着会盖住 AppDialog
      emit("close");
      await dialogStore.alert(t("settings.asr.hotwordLibrary.dialog.loadFailed"));
    } finally {
      loading.value = false;
    }
  },
);

const handleClose = () => emit("close");

/** 确认：把**副本**交给宿主（拷贝式语义，库中后续改动不回传）。 */
const confirm = () => {
  emit(
    "add",
    [...picked.value].map((w) => ({ ...w })),
  );
  emit("close");
};
</script>

<style scoped>
/* 与 GlobalHotwordDialog / HotwordLibraryDialog 同款进出场（scoped，故必须各文件自带一份） */
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

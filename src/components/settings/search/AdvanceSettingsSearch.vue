<template>
  <div :class="hasQuery ? 'flex min-h-0 flex-1 flex-col' : 'shrink-0'">
    <div class="shrink-0 px-2 pb-3">
      <div
        class="group focus-within:border-brand/70 flex items-center gap-2.5 rounded-xl border border-white/15 bg-black/25 px-3.5 py-2.5 backdrop-blur-md transition-all duration-200 focus-within:bg-black/35 focus-within:shadow-[0_0_16px_rgba(121,217,255,0.18)]"
      >
        <Search
          :size="16"
          class="group-focus-within:text-brand shrink-0 text-white/50 transition-colors"
        />
        <input
          ref="inputRef"
          v-model="query"
          type="text"
          autocomplete="off"
          spellcheck="false"
          :placeholder="$t('advance.search.placeholder')"
          :aria-label="$t('advance.search.placeholder')"
          class="min-w-0 flex-1 bg-transparent text-sm text-white placeholder-white/40 outline-none"
          @keydown="handleInputKeydown"
        />

        <div class="flex shrink-0 items-center gap-2">
          <span
            v-if="hasQuery && loadingDynamic"
            class="inline-flex items-center gap-1 text-[11px] text-white/55"
          >
            <Loader2 :size="12" class="animate-spin" />
            <span class="hidden sm:inline">{{ $t("advance.search.loadingDynamic") }}</span>
          </span>

          <span
            v-if="hasQuery"
            class="rounded-full border border-white/10 bg-white/5 px-2.5 py-0.5 text-[11px] font-medium text-white/75 tabular-nums"
          >
            {{
              totalMatches > results.length
                ? $t("advance.search.resultCountCapped", {
                    shown: results.length,
                    total: totalMatches,
                  })
                : $t("advance.search.resultCount", { count: totalMatches })
            }}
          </span>
          <span v-else class="hidden text-[11px] text-white/40 md:inline">
            {{ $t("advance.search.shortcutHint") }}
          </span>

          <button
            v-if="hasQuery"
            type="button"
            class="flex h-6 w-6 cursor-pointer items-center justify-center rounded-md text-white/55 transition-colors hover:bg-white/15 hover:text-white"
            :title="$t('advance.search.clear')"
            :aria-label="$t('advance.search.clear')"
            @click="clearQuery"
          >
            <X :size="14" />
          </button>
        </div>
      </div>
    </div>

    <div v-if="hasQuery" class="custom-scrollbar min-h-0 flex-1 overflow-y-auto">
      <div class="space-y-2 p-2">
        <div
          v-if="results.length === 0"
          class="flex flex-col items-center justify-center rounded-2xl border border-white/10 bg-white/5 px-6 py-12 text-center"
        >
          <SearchX :size="30" class="mb-2.5 text-white/30" />
          <p class="text-sm font-semibold text-white/85">
            {{ $t("advance.search.emptyTitle") }}
          </p>
          <p class="mt-1 max-w-md text-xs leading-relaxed text-white/50">
            {{ $t("advance.search.emptyHint") }}
          </p>
          <button
            type="button"
            class="mt-4 cursor-pointer rounded-lg border border-white/15 bg-white/10 px-3.5 py-1.5 text-xs font-medium text-white/85 transition-colors hover:bg-white/20"
            @click="clearQuery"
          >
            {{ $t("advance.search.clear") }}
          </button>
        </div>

        <button
          v-for="(entry, idx) in results"
          :key="entry.item.id"
          :ref="(el) => setResultEl(idx, el)"
          type="button"
          class="group flex w-full cursor-pointer items-center justify-between gap-4 rounded-xl border px-4 py-3 text-left transition-all duration-150"
          :class="
            idx === activeIndex
              ? 'border-brand/60 bg-brand/15 shadow-[0_0_14px_rgba(121,217,255,0.14)]'
              : 'border-white/10 bg-white/5 hover:border-white/25 hover:bg-white/10'
          "
          @mouseenter="activeIndex = idx"
          @click="selectEntry(entry.item)"
        >
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center gap-1.5">
              <template v-for="(crumb, cIdx) in entry.item.breadcrumbs" :key="cIdx">
                <ChevronRight v-if="cIdx > 0" :size="12" class="shrink-0 text-white/35" />
                <span class="text-brand/90 text-[11px] font-medium">
                  <template v-for="(seg, sIdx) in splitHighlightSegments(crumb, query)" :key="sIdx">
                    <mark v-if="seg.matched" class="bg-brand/30 rounded px-0.5 text-white">{{
                      seg.text
                    }}</mark>
                    <span v-else>{{ seg.text }}</span>
                  </template>
                </span>
              </template>

              <code
                v-if="entry.item.rawKey"
                class="rounded border border-white/10 bg-black/35 px-1.5 py-0.5 font-mono text-[10px] text-white/70"
              >
                <template
                  v-for="(seg, sIdx) in splitHighlightSegments(entry.item.rawKey, query)"
                  :key="sIdx"
                >
                  <mark v-if="seg.matched" class="bg-brand/35 rounded px-0.5 text-white">{{
                    seg.text
                  }}</mark>
                  <span v-else>{{ seg.text }}</span>
                </template>
              </code>

              <span
                v-if="entry.item.restartRequired"
                class="rounded border border-amber-400/30 bg-amber-500/10 px-1.5 py-0.5 text-[10px] text-amber-300"
              >
                {{ $t("advance.search.restartTag") }}
              </span>
            </div>

            <div class="mt-1 truncate text-sm font-semibold text-white">
              <template
                v-for="(seg, sIdx) in splitHighlightSegments(entry.item.title, query)"
                :key="sIdx"
              >
                <mark v-if="seg.matched" class="bg-brand/35 rounded px-0.5 text-white">{{
                  seg.text
                }}</mark>
                <span v-else>{{ seg.text }}</span>
              </template>
            </div>

            <p
              v-if="entry.item.description"
              class="mt-0.5 line-clamp-2 text-xs leading-relaxed text-white/60"
            >
              <template
                v-for="(seg, sIdx) in splitHighlightSegments(entry.item.description, query)"
                :key="sIdx"
              >
                <mark v-if="seg.matched" class="bg-brand/25 rounded px-0.5 text-white/95">{{
                  seg.text
                }}</mark>
                <span v-else>{{ seg.text }}</span>
              </template>
            </p>
          </div>

          <span
            class="flex shrink-0 items-center gap-1 rounded-lg border px-2.5 py-1 text-xs font-medium transition-colors"
            :class="
              idx === activeIndex
                ? 'border-brand/55 bg-brand/25 text-white'
                : 'group-hover:border-brand/45 border-white/10 bg-white/5 text-white/65 group-hover:text-white'
            "
          >
            {{ $t("advance.search.jumpLabel") }}
            <ArrowRight :size="13" />
          </span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  computed,
  nextTick,
  onActivated,
  onBeforeUnmount,
  onDeactivated,
  onMounted,
  ref,
  watch,
  type ComponentPublicInstance,
} from "vue";
import { ArrowRight, ChevronRight, Loader2, Search, SearchX, X } from "@lucide/vue";
import { useAdvanceJump } from "./advanceJump";
import {
  splitHighlightSegments,
  useSettingsSearch,
  type SettingsSearchItem,
} from "./useSettingsSearch";

const query = defineModel<string>({ default: "" });
const { jumpTo } = useAdvanceJump();

const activeIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const resultRefs = ref<Array<HTMLElement | null>>([]);
let isActive = true;

const hasQuery = computed(() => query.value.trim().length > 0);
const { results, totalMatches, loadingDynamic, ensureCatalogData } = useSettingsSearch(query);

function setResultEl(index: number, el: Element | ComponentPublicInstance | null) {
  resultRefs.value[index] = el instanceof HTMLElement ? el : null;
}

watch(results, (list) => {
  resultRefs.value.length = list.length;
  if (activeIndex.value >= list.length) {
    activeIndex.value = 0;
  }
});

watch(query, () => {
  activeIndex.value = 0;
});

function clearQuery() {
  query.value = "";
  activeIndex.value = 0;
  void nextTick(() => inputRef.value?.focus());
}

function selectEntry(item: SettingsSearchItem) {
  activeIndex.value = 0;
  inputRef.value?.blur();
  void jumpTo(item.target);
}

function scrollActiveIntoView() {
  void nextTick(() => {
    resultRefs.value[activeIndex.value]?.scrollIntoView({ block: "nearest" });
  });
}

function handleInputKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown") {
    if (!hasQuery.value || results.value.length === 0) return;
    event.preventDefault();
    activeIndex.value = (activeIndex.value + 1) % results.value.length;
    scrollActiveIntoView();
    return;
  }

  if (event.key === "ArrowUp") {
    if (!hasQuery.value || results.value.length === 0) return;
    event.preventDefault();
    activeIndex.value = (activeIndex.value - 1 + results.value.length) % results.value.length;
    scrollActiveIntoView();
    return;
  }

  if (event.key === "Enter") {
    if (!hasQuery.value || results.value.length === 0) return;
    event.preventDefault();
    const target = results.value[activeIndex.value] ?? results.value[0];
    if (target) {
      selectEntry(target.item);
    }
    return;
  }

  if (event.key === "Escape") {
    if (hasQuery.value) {
      event.preventDefault();
      event.stopPropagation();
      query.value = "";
      activeIndex.value = 0;
    } else {
      inputRef.value?.blur();
    }
  }
}

function handleGlobalKeydown(event: KeyboardEvent) {
  if (!isActive || !inputRef.value || inputRef.value.getClientRects().length === 0) return;
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    inputRef.value.focus();
    inputRef.value.select();
  }
}

onMounted(() => {
  isActive = true;
  void ensureCatalogData();
  window.addEventListener("keydown", handleGlobalKeydown);
});

onActivated(() => {
  isActive = true;
});

onDeactivated(() => {
  isActive = false;
});

onBeforeUnmount(() => {
  isActive = false;
  window.removeEventListener("keydown", handleGlobalKeydown);
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="flex shrink-0 items-center gap-2 px-3 pt-1 pb-2 text-[0.66rem]">
      <span class="text-white/40">{{ $t("game.director.tools.count", { n: pairs.length }) }}</span>
      <button
        class="ml-auto inline-flex shrink-0 items-center gap-1 rounded-md border border-white/12 px-2 py-1 text-white/60 transition-colors hover:bg-white/8 hover:text-white/85 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="store.loadingTools || !store.available"
        @click="reload"
      >
        <RefreshCw :size="11" :class="{ 'animate-spin': store.loadingTools }" />
        {{ $t("game.director.context.refresh") }}
      </button>
    </div>

    <div class="min-h-0 flex-1 space-y-3 overflow-y-auto px-3 py-1">
      <section v-if="activeEntries.length > 0">
        <p class="mb-1.5 text-[0.66rem] font-medium text-white/40">
          {{ $t("game.director.tools.currentRound") }}
        </p>
        <ul class="space-y-1.5">
          <li
            v-for="entry in activeEntries"
            :key="entry.id"
            class="rounded-xl border border-white/10 bg-black/20 px-2.5 py-1.5"
          >
            <div class="flex items-center gap-1.5 text-[0.68rem]">
              <span
                class="shrink-0 rounded-full border px-1.5 py-px text-[0.6rem]"
                :class="statusClass(entry.status)"
              >
                {{ statusText(entry.status) }}
              </span>
              <span class="truncate text-sky-300/85">{{ entry.tool }}</span>
              <span
                v-if="argsSummary(entry)"
                class="truncate font-mono text-[0.6rem] text-white/30"
                :title="argsFull(entry)"
              >
                {{ argsSummary(entry) }}
              </span>
            </div>
            <pre
              v-if="entry.output || entry.error"
              class="mt-1 max-h-24 overflow-y-auto rounded bg-black/30 p-1.5 font-mono text-[0.62rem] break-all whitespace-pre-wrap text-white/60"
              >{{ entry.error || entry.output }}</pre>
          </li>
        </ul>
      </section>

      <p v-if="!store.available" class="px-2 py-8 text-center text-[0.78rem] text-white/35">
        {{ $t("game.director.unavailable.title") }}
      </p>
      <p
        v-else-if="store.loadingTools && pairs.length === 0 && activeEntries.length === 0"
        class="px-2 py-8 text-center text-[0.78rem] text-white/35"
      >
        {{ $t("game.director.context.loading") }}
      </p>
      <div
        v-else-if="pairs.length === 0 && activeEntries.length === 0"
        class="px-4 py-10 text-center"
      >
        <Wrench :size="24" class="mx-auto mb-2 text-white/20" />
        <p class="text-[0.78rem] text-white/45">{{ $t("game.director.empty.tools") }}</p>
        <p class="mt-1 text-[0.68rem] leading-relaxed text-white/30">
          {{ $t("game.director.empty.toolsHint") }}
        </p>
      </div>

      <section v-if="pairs.length > 0">
        <p v-if="activeEntries.length > 0" class="mb-1.5 text-[0.66rem] font-medium text-white/40">
          {{ $t("game.director.tools.history") }}
        </p>
        <ul class="space-y-2">
          <li
            v-for="(pair, index) in pairs"
            :key="`${pair.call.id}-${index}`"
            class="rounded-xl border border-white/10 bg-black/20 px-2.5 py-2"
          >
            <div class="flex items-center gap-1.5 text-[0.68rem]">
              <span class="text-sky-300/85">{{ pair.call.function.name }}</span>
              <span class="truncate font-mono text-[0.62rem] text-white/30">{{
                pair.call.id
              }}</span>
              <span
                v-if="pair.result"
                class="ml-auto shrink-0 rounded-full border px-1.5 py-px text-[0.6rem]"
                :class="
                  okOf(pair.result)
                    ? 'border-brand/40 text-brand'
                    : 'border-red-400/40 text-red-300'
                "
              >
                {{
                  okOf(pair.result)
                    ? $t("game.director.tools.ok")
                    : $t("game.director.tools.failed")
                }}
              </span>
              <span v-else class="ml-auto shrink-0 text-[0.6rem] text-white/30">
                {{ $t("game.director.tools.noResult") }}
              </span>
            </div>
            <p class="mt-1.5 mb-0.5 text-[0.62rem] text-white/30">
              {{ $t("game.director.tools.args") }}
            </p>
            <pre
              class="max-h-40 overflow-y-auto rounded bg-black/30 p-1.5 font-mono text-[0.62rem] break-all whitespace-pre-wrap text-white/60"
              >{{ prettyJson(pair.call.function.arguments) }}</pre>
            <template v-if="pair.result">
              <p class="mt-1.5 mb-0.5 text-[0.62rem] text-white/30">
                {{ $t("game.director.tools.result") }}
              </p>
              <pre
                class="max-h-48 overflow-y-auto rounded bg-black/30 p-1.5 font-mono text-[0.62rem] break-all whitespace-pre-wrap text-white/60"
                >{{ pair.result.content }}</pre>
            </template>
          </li>
        </ul>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, watch } from "vue";
import { RefreshCw, Wrench } from "@lucide/vue";
import {
  isDirectorToolResultOk,
  pairDirectorToolHistory,
  summarizeDirectorArgs,
  useDirectorStore,
  type DirectorActiveTool,
} from "@/stores/modules/director";
import type { DirectorMessage } from "@/api/services/director";
import { i18n } from "@/locales";

const store = useDirectorStore();
const t = i18n.global.t;
const activeEntries = computed(() =>
  Object.entries(store.activeTools).map(([id, entry]) => ({ id, ...entry })),
);
const pairs = computed(() => pairDirectorToolHistory(store.toolHistory));

const statusClass = (status: DirectorActiveTool["status"]) => {
  if (status === "running") return "border-amber-400/40 text-amber-300";
  if (status === "ok") return "border-brand/40 text-brand";
  if (status === "error") return "border-red-400/40 text-red-300";
  return "border-white/12 text-white/40";
};
const statusText = (status: DirectorActiveTool["status"]) => {
  if (status === "running") return t("game.director.tools.running");
  if (status === "ok") return t("game.director.tools.ok");
  if (status === "error") return t("game.director.tools.failed");
  return t("game.director.tools.noResult");
};
const argsSummary = (entry: DirectorActiveTool) => summarizeDirectorArgs(entry.args);
const argsFull = (entry: DirectorActiveTool) =>
  summarizeDirectorArgs(entry.args, Number.MAX_SAFE_INTEGER) || "—";
const okOf = (result: DirectorMessage) => isDirectorToolResultOk(result.content);
const prettyJson = (raw: string) => {
  if (!raw) return "—";
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
};

function reload() {
  void store.loadToolHistory();
}

watch(
  () => [store.ready, store.available, store.saveId] as const,
  ([ready, available]) => {
    if (ready && available) reload();
  },
  { immediate: true },
);
</script>

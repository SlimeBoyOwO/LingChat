<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="shrink-0 space-y-2 border-b border-white/8 px-3 pt-1 pb-3">
      <div class="flex items-center gap-2 text-[0.72rem] text-white/70">
        <span>{{ $t("game.director.draft.modeEnabled") }}</span>
        <button
          type="button"
          role="switch"
          :aria-checked="store.directorMode.enabled"
          class="relative ml-auto h-5 w-9 shrink-0 rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-40"
          :class="store.directorMode.enabled ? 'bg-brand/70' : 'bg-white/15'"
          :disabled="modeBusy || !store.available"
          @click="toggleMode"
        >
          <span
            class="absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all"
            :class="store.directorMode.enabled ? 'left-[1.125rem]' : 'left-0.5'"
          ></span>
        </button>
      </div>
      <div class="flex items-center gap-2 text-[0.72rem] text-white/70">
        <span>{{ $t("game.director.draft.intervalPrefix") }}</span>
        <input
          v-model.number="intervalInput"
          type="number"
          min="1"
          max="20"
          class="focus:border-brand/50 w-12 rounded-md border border-white/12 bg-black/20 px-1.5 py-1 text-center text-[0.72rem] text-white/85 outline-none disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="modeBusy || !store.available"
          @input="intervalDirty = true"
          @change="commitInterval"
        />
        <span>{{ $t("game.director.draft.intervalSuffix") }}</span>
      </div>
      <p class="text-[0.64rem] text-white/35">{{ sinceLastText }}</p>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-3 py-2">
      <p v-if="!store.available" class="px-2 py-8 text-center text-[0.78rem] text-white/35">
        {{ $t("game.director.unavailable.title") }}
      </p>
      <template v-else>
        <div v-if="draft" class="space-y-2">
          <div class="flex items-center gap-2 text-[0.68rem]">
            <span
              class="shrink-0 rounded-full border px-1.5 py-px text-[0.62rem]"
              :class="statusClass(draft.status)"
            >
              {{ statusText(draft.status) }}
            </span>
            <span class="text-white/35">
              {{ $t("game.director.draft.eventCount", { n: draft.events.length }) }}
            </span>
            <button
              class="ml-auto inline-flex shrink-0 items-center gap-1 rounded-md border border-white/12 px-2 py-1 text-white/60 transition-colors hover:bg-white/8 hover:text-white/85 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="modeBusy || draftBusy"
              @click="reload"
            >
              <RefreshCw :size="11" :class="{ 'animate-spin': store.loadingMode }" />
              {{ $t("game.director.draft.refresh") }}
            </button>
          </div>
          <p
            v-if="draft.status === 'failed' && draft.error"
            class="rounded-lg border border-red-400/30 bg-red-400/8 px-2 py-1.5 text-[0.66rem] leading-relaxed text-red-300/85"
          >
            {{ draft.error }}
          </p>
          <p
            v-if="draft.events.length === 0"
            class="px-2 py-6 text-center text-[0.72rem] text-white/35"
          >
            {{
              draft.status === "generating"
                ? $t("game.director.draft.statusGenerating")
                : $t("game.director.draft.noEvents")
            }}
          </p>
          <ul v-else class="space-y-1.5">
            <li
              v-for="(ev, index) in draft.events"
              :key="index"
              class="rounded-lg border border-white/10 bg-black/20 px-2.5 py-1.5"
            >
              <div class="flex items-center gap-1.5 text-[0.68rem]">
                <span class="w-4 shrink-0 text-right text-white/30">{{ index + 1 }}</span>
                <span class="truncate text-sky-300/85">{{ ev.event }}</span>
              </div>
              <p
                v-if="argsSummary(ev)"
                class="mt-1 truncate pl-5 font-mono text-[0.62rem] text-white/40"
                :title="argsFull(ev)"
              >
                {{ argsSummary(ev) }}
              </p>
            </li>
          </ul>
        </div>
        <div v-else class="px-4 py-10 text-center">
          <Clapperboard :size="24" class="mx-auto mb-2 text-white/20" />
          <p class="text-[0.78rem] text-white/45">{{ $t("game.director.draft.empty") }}</p>
          <p class="mt-1 text-[0.68rem] leading-relaxed text-white/30">
            {{ $t("game.director.draft.emptyHint") }}
          </p>
        </div>
      </template>
    </div>

    <div
      v-if="store.applyResult || store.modeError || actionableDraft"
      class="shrink-0 space-y-2 border-t border-white/8 px-3 py-2"
    >
      <p
        v-if="store.applyResult"
        class="text-[0.66rem] leading-relaxed"
        :class="store.applyResult.ok ? 'text-brand' : 'text-red-300'"
      >
        {{
          store.applyResult.ok
            ? $t("game.director.draft.applySuccess", { n: store.applyResult.applied })
            : $t("game.director.draft.applyFailed", {
                n: store.applyResult.applied,
                error: store.applyResult.error || "",
              })
        }}
      </p>
      <p v-if="store.modeError" class="text-[0.66rem] leading-relaxed text-red-300">
        {{ $t("game.director.draft.operationFailed", { error: store.modeError }) }}
      </p>
      <div v-if="actionableDraft" class="flex gap-2">
        <button
          v-if="draft?.status === 'pending'"
          class="border-brand/40 bg-brand/15 text-brand hover:bg-brand/25 flex-1 rounded-lg border px-2 py-1.5 text-[0.72rem] transition-colors disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="draftBusy"
          @click="confirmPlay"
        >
          {{
            store.confirming
              ? $t("game.director.draft.statusApplying")
              : $t("game.director.draft.confirm")
          }}
        </button>
        <button
          class="rounded-lg border border-white/12 px-3 py-1.5 text-[0.72rem] text-white/60 transition-colors hover:bg-white/8 hover:text-white/85 disabled:cursor-not-allowed disabled:opacity-40"
          :class="{ 'flex-1': draft?.status === 'failed' }"
          :disabled="draftBusy"
          @click="discard"
        >
          {{
            store.discarding
              ? $t("game.director.draft.discarding")
              : $t("game.director.draft.discard")
          }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Clapperboard, RefreshCw } from "@lucide/vue";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import { useGameStore } from "@/stores/modules/game";
import { summarizeDirectorArgs, useDirectorStore } from "@/stores/modules/director";
import type { DirectorDraft, DirectorDraftEvent } from "@/api/services/director";

const { t } = useI18n();
const store = useDirectorStore();
const gameStore = useGameStore();
const dialogStore = useDialogStore();
const draft = computed(() => store.draft);
const intervalInput = ref(store.directorMode.interval);
const intervalDirty = ref(false);
const dialogOpening = ref(false);
const modeBusy = computed(
  () => store.loadingMode || store.savingMode || store.confirming || store.discarding,
);
const draftBusy = computed(() => store.confirming || store.discarding || dialogOpening.value);
const actionableDraft = computed(
  () => draft.value?.status === "pending" || draft.value?.status === "failed",
);

watch(
  () => store.directorMode.interval,
  (value) => {
    if (!intervalDirty.value) intervalInput.value = value;
  },
);

const currentSeq = computed(() => {
  let max = 0;
  for (const message of gameStore.dialogHistory) {
    if (typeof message.userMessageSeq === "number" && message.userMessageSeq > max) {
      max = message.userMessageSeq;
    }
  }
  return max;
});
const sinceLastText = computed(() => {
  const last = store.directorMode.lastGeneratedSeq;
  if (last == null) return t("game.director.draft.notGenerated");
  return t("game.director.draft.sinceLast", { n: Math.max(0, currentSeq.value - last) });
});
const STATUS_LABELS: Record<DirectorDraft["status"], string> = {
  idle: "game.director.draft.statusIdle",
  generating: "game.director.draft.statusGenerating",
  pending: "game.director.draft.statusPending",
  applying: "game.director.draft.statusApplying",
  failed: "game.director.draft.statusFailed",
};
const statusText = (status: DirectorDraft["status"]) => t(STATUS_LABELS[status]);
const statusClass = (status: DirectorDraft["status"]) => {
  if (status === "generating" || status === "applying") return "border-amber-400/40 text-amber-300";
  if (status === "pending") return "border-brand/40 text-brand";
  if (status === "failed") return "border-red-400/40 text-red-300";
  return "border-white/12 text-white/40";
};
const argsSummary = (event: DirectorDraftEvent) => summarizeDirectorArgs(event.params);
const argsFull = (event: DirectorDraftEvent) =>
  summarizeDirectorArgs(event.params, Number.MAX_SAFE_INTEGER) || "—";

function toggleMode() {
  if (modeBusy.value) return;
  void store.setMode(!store.directorMode.enabled);
}

async function commitInterval() {
  if (modeBusy.value) return;
  const raw = Number(intervalInput.value);
  const previous = store.directorMode.interval;
  const value = Number.isFinite(raw) ? Math.min(20, Math.max(1, Math.round(raw))) : previous;
  intervalInput.value = value;
  intervalDirty.value = false;
  if (value === previous) return;
  const saved = await store.setMode(store.directorMode.enabled, value);
  if (!saved && !intervalDirty.value) intervalInput.value = store.directorMode.interval;
}

function reload() {
  if (!modeBusy.value && !draftBusy.value) void store.loadMode();
}

async function confirmPlay() {
  if (draftBusy.value || draft.value?.status !== "pending") return;
  dialogOpening.value = true;
  try {
    if (!(await dialogStore.confirm(t("game.director.draft.confirmDialog")))) return;
    await store.confirmDraft();
  } finally {
    dialogOpening.value = false;
  }
}

async function discard() {
  if (draftBusy.value || !actionableDraft.value) return;
  await store.discardDraft();
}

let timer: ReturnType<typeof setTimeout> | null = null;
let pollVersion = 0;
function stopPolling() {
  pollVersion++;
  if (timer) clearTimeout(timer);
  timer = null;
}

/** 异步草案只在上一轮读取结束后调度下一次，卸载时使回调失效。 */
function syncPolling() {
  stopPolling();
  const status = store.draft?.status;
  if (status !== "generating" && status !== "applying") return;
  const version = pollVersion;
  timer = setTimeout(async () => {
    timer = null;
    if (version !== pollVersion) return;
    await store.loadMode(true);
    if (version === pollVersion) syncPolling();
  }, 1500);
}

watch(
  () => store.saveId,
  () => {
    intervalDirty.value = false;
    intervalInput.value = store.directorMode.interval;
  },
);
watch(
  () => [store.ready, store.available] as const,
  ([ready, available]) => {
    if (ready && available) void store.loadMode();
  },
  { immediate: true },
);
watch(
  () => [store.draft?.status, store.confirming, store.discarding, store.savingMode] as const,
  syncPolling,
  { immediate: true },
);
onBeforeUnmount(stopPolling);
</script>

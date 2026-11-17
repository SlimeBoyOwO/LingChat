<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="flex shrink-0 items-center gap-2 px-3 pt-1 pb-2 text-[0.66rem]">
      <span class="text-white/40">
        {{ $t("game.director.tools.count", { n: pairs.length }) }}
      </span>
      <button
        class="ml-auto inline-flex shrink-0 items-center gap-1 rounded-md border border-white/12 px-2 py-1 text-white/60 transition-colors hover:bg-white/8 hover:text-white/85 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="store.loadingTools || !store.available"
        @click="reload"
      >
        <RefreshCw :size="11" :class="{ 'animate-spin': store.loadingTools }" />
        {{ $t("game.director.context.refresh") }}
      </button>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-3 py-1">
      <p v-if="!store.available" class="px-2 py-8 text-center text-[0.78rem] text-white/35">
        {{ $t("game.director.unavailable.title") }}
      </p>
      <p
        v-else-if="store.loadingTools && pairs.length === 0"
        class="px-2 py-8 text-center text-[0.78rem] text-white/35"
      >
        {{ $t("game.director.context.loading") }}
      </p>
      <div v-else-if="pairs.length === 0" class="px-4 py-10 text-center">
        <Wrench :size="24" class="mx-auto mb-2 text-white/20" />
        <p class="text-[0.78rem] text-white/45">{{ $t("game.director.empty.tools") }}</p>
        <p class="mt-1 text-[0.68rem] leading-relaxed text-white/30">
          {{ $t("game.director.empty.toolsHint") }}
        </p>
      </div>

      <ul v-else class="space-y-2">
        <li
          v-for="(pair, index) in pairs"
          :key="`${pair.call.id}-${index}`"
          class="rounded-xl border border-white/10 bg-black/20 px-2.5 py-2"
        >
          <div class="flex items-center gap-1.5 text-[0.68rem]">
            <span class="text-sky-300/85">{{ pair.call.function.name }}</span>
            <span class="truncate font-mono text-[0.62rem] text-white/30">{{ pair.call.id }}</span>
            <span
              v-if="pair.result"
              class="ml-auto shrink-0 rounded-full border px-1.5 py-px text-[0.6rem]"
              :class="
                okOf(pair.result) ? 'border-brand/40 text-brand' : 'border-red-400/40 text-red-300'
              "
            >
              {{
                okOf(pair.result) ? $t("game.director.tools.ok") : $t("game.director.tools.failed")
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
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, watch } from "vue";
import { RefreshCw, Wrench } from "@lucide/vue";
import { useDirectorStore } from "@/stores/modules/director";
import type { DirectorMessage, DirectorToolCall } from "@/api/services/director";

const store = useDirectorStore();

/** 工具调用与结果的配对：assistant.tool_calls + 紧随其后的 role=tool 消息 */
const pairs = computed(() => {
  const results = new Map<string, DirectorMessage>();
  for (const m of store.toolHistory) {
    if (m.role === "tool" && m.toolCallId) results.set(m.toolCallId, m);
  }
  const out: { call: DirectorToolCall; result: DirectorMessage | undefined }[] = [];
  for (const m of store.toolHistory) {
    if (m.role !== "assistant" || !m.toolCalls) continue;
    for (const call of m.toolCalls) out.push({ call, result: results.get(call.id) });
  }
  return out;
});

/** 工具结果由 ToolExecutor 统一编码成 {"ok":false,...}，不抛异常 */
function okOf(result: DirectorMessage): boolean {
  const raw = result.content ?? "";
  try {
    const parsed = JSON.parse(raw) as { ok?: boolean };
    return parsed.ok !== false;
  } catch {
    return true;
  }
}

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

onMounted(() => {
  if (store.ready && store.available) reload();
});
watch(
  () => store.ready,
  (ready) => {
    if (ready && store.available) reload();
  },
);
</script>

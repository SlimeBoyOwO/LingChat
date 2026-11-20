<template>
  <details class="group max-w-full text-[0.66rem]">
    <summary
      class="inline-flex max-w-full cursor-pointer items-center gap-1.5 rounded-md border border-white/10 bg-black/20 px-2 py-0.5 text-white/45 transition-colors select-none hover:bg-white/8 hover:text-white/70"
    >
      <Wrench :size="11" class="shrink-0" />
      <span class="truncate">{{ summary }}</span>
      <span class="shrink-0 text-white/25">{{ $t("game.director.tools.expand") }}</span>
    </summary>
    <ul class="mt-1 space-y-1 pl-1">
      <li
        v-for="c in view"
        :key="c.id"
        class="rounded-md border border-white/10 bg-black/25 px-2 py-1"
      >
        <div class="flex items-center gap-1.5">
          <span
            class="shrink-0 rounded-full border px-1.5 py-px text-[0.6rem]"
            :class="statusClass(c.status)"
          >
            {{ statusText(c.status) }}
          </span>
          <span class="truncate text-sky-300/85">{{ c.tool }}</span>
        </div>
        <pre
          v-if="c.output || c.error"
          class="mt-1 max-h-32 overflow-y-auto rounded bg-black/30 p-1.5 font-mono text-[0.62rem] break-all whitespace-pre-wrap text-white/60"
          >{{ c.error || c.output }}</pre>
      </li>
    </ul>
  </details>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { Wrench } from "@lucide/vue";
import {
  directorToolFailure,
  pairDirectorToolHistory,
  summarizeDirectorArgs,
  useDirectorStore,
  type DirectorActiveTool,
} from "@/stores/modules/director";
import type { DirectorMessage } from "@/api/services/director";
import { i18n } from "@/locales";

const t = i18n.global.t;
type CallView = DirectorActiveTool & { id: string };
const props = defineProps<{
  message?: DirectorMessage;
  messageIndex?: number;
  calls?: CallView[];
}>();
const store = useDirectorStore();

const archivedCalls = computed<CallView[]>(() => {
  const message = props.message;
  if (!message?.toolCalls) return [];
  const index = props.messageIndex ?? store.messages.indexOf(message);
  if (index < 0) return [];
  const pairs = pairDirectorToolHistory(store.messages.slice(index));
  const results = new Map<string, (typeof pairs)[number]["result"]>();
  for (const pair of pairs) {
    if (!results.has(pair.call.id)) results.set(pair.call.id, pair.result);
  }
  return message.toolCalls.map((call) => {
    const result = results.get(call.id);
    const failure = result ? directorToolFailure(result.content) : null;
    return {
      id: call.id,
      tool: call.function.name,
      args: safeParseArgs(call.function.arguments),
      status: result ? (failure ? "error" : "ok") : "pending",
      output: result?.content ?? null,
      error: failure,
    };
  });
});

const view = computed(() => props.calls ?? archivedCalls.value);
const summary = computed(() => {
  const list = view.value;
  const first = list[0];
  if (!first) return "";
  const args = summarizeDirectorArgs(first.args);
  const head = t("game.director.tools.called", { tool: first.tool });
  if (list.length === 1) return args ? `${head} ${args}` : head;
  return `${head} ${t("game.director.tools.more", { n: list.length - 1 })}`;
});

function safeParseArgs(raw: string): Record<string, unknown> {
  if (!raw) return {};
  try {
    const value = JSON.parse(raw) as unknown;
    return value && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : { value };
  } catch {
    return { _: raw };
  }
}

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
</script>

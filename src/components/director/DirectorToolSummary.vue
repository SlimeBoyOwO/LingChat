<template>
  <details class="group max-w-full text-[0.66rem]">
    <!-- 一行紧凑摘要：N 个工具调用，展开看每个的结果 -->
    <summary
      class="inline-flex max-w-full cursor-pointer items-center gap-1.5 rounded-md border border-white/10 bg-black/20 px-2 py-0.5 text-white/45 transition-colors hover:bg-white/8 hover:text-white/70 select-none"
    >
      <Wrench :size="11" class="shrink-0" />
      <span class="truncate">
        {{ summary }}
      </span>
      <span class="shrink-0 text-white/25">{{ $t("game.director.tools.expand") }}</span>
    </summary>
    <ul class="mt-1 space-y-1 pl-1">
      <li v-for="c in calls" :key="c.id" class="rounded-md border border-white/10 bg-black/25 px-2 py-1">
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
/**
 * 聊天流里的工具调用紧凑摘要（最小视觉占用）：
 * 收起时只有一行「已调用 xxx…」+ 展开提示，展开后按调用逐条看结果。
 * 两种数据源：本轮进行中的 activeTools（store 实时）与已归档进
 * assistant 消息的 toolCalls（结构同后端落库）。
 */
import { computed } from "vue";
import { Wrench } from "@lucide/vue";
import { useDirectorStore, summarizeDirectorArgs } from "@/stores/modules/director";
import type { DirectorActiveTool } from "@/stores/modules/director";
import type { DirectorMessage } from "@/api/services/director";
import { i18n } from "@/locales";

const t = i18n.global.t;

type CallView = DirectorActiveTool & { id: string; tool: string };

const props = defineProps<{
  /** 已归档进 assistant 消息的调用（渲染状态从 role=tool 结果反推） */
  message?: DirectorMessage;
  /** 本轮进行中的调用（id + 实时状态），优先于 message */
  calls?: CallView[];
}>();

const store = useDirectorStore();

/** 本轮实时条目 */
const activeCalls = computed<CallView[]>(() => {
  if (!props.calls) return [];
  return props.calls.flatMap((c) => {
    const entry = store.activeTools[c.id];
    if (!entry) return [];
    // 事件里只带 id，实际状态以 store 实时条目为准
    return [{ ...entry, id: c.id, tool: entry.tool }];
  });
});

/** 已归档条目：toolCalls + 消息流里按 toolCallId 配对的结果 */
const archivedCalls = computed<CallView[]>(() => {
  if (!props.message?.toolCalls) return [];
  const results = new Map<string, DirectorMessage>();
  const list = store.messages;
  const idx = list.indexOf(props.message);
  for (const m of idx >= 0 ? list.slice(idx + 1) : []) {
    if (m.role === "tool" && m.toolCallId && !results.has(m.toolCallId)) {
      results.set(m.toolCallId, m);
    }
  }
  return props.message.toolCalls.map((call) => {
    const result = results.get(call.id);
    const failed = result ? !isResultOk(result.content) : false;
    return {
      id: call.id,
      tool: call.function.name,
      args: safeParseArgs(call.function.arguments),
      status: result ? (failed ? "error" : "ok") : "pending",
      output: result?.content ?? null,
      error: failed ? (result?.content ?? null) : null,
    } satisfies CallView;
  });
});

const view = computed(() => (props.calls ? activeCalls.value : archivedCalls.value));

/** 一行摘要：已调用 director_xxx（多个时只列第一个 + 计数） */
const summary = computed(() => {
  const list = view.value;
  if (list.length === 0) return "";
  const first = list[0];
  const args = summarizeDirectorArgs(first.args);
  const head = `${t("game.director.tools.called", { tool: first.tool })}`;
  if (list.length === 1) return args ? `${head} ${args}` : head;
  return `${head} ${t("game.director.tools.more", { n: list.length - 1 })}`;
});

function safeParseArgs(raw: string): Record<string, unknown> {
  if (!raw) return {};
  try {
    return JSON.parse(raw) as Record<string, unknown>;
  } catch {
    return { _: raw };
  }
}

/** 工具结果由 ToolExecutor 统一编码成 {"ok":false,...}；非 JSON 视为成功 */
function isResultOk(content: string): boolean {
  try {
    const parsed = JSON.parse(content) as { ok?: boolean };
    return parsed.ok !== false;
  } catch {
    return true;
  }
}

const statusClass = (status: DirectorActiveTool["status"]) =>
  status === "running"
    ? "border-amber-400/40 text-amber-300"
    : status === "ok"
      ? "border-brand/40 text-brand"
      : "border-red-400/40 text-red-300";

const statusText = (status: DirectorActiveTool["status"]) =>
  status === "running"
    ? t("game.director.tools.running")
    : status === "ok"
      ? t("game.director.tools.ok")
      : status === "error"
        ? t("game.director.tools.failed")
        : t("game.director.tools.noResult");
</script>

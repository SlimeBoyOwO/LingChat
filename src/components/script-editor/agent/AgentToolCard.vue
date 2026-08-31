<template>
  <div class="rounded-lg border" :class="[tone.border, tone.bg]">
    <!-- 单行紧凑头部：图标 + 工具名 + 审批按钮（仅待审批）+ 详情切换 + 状态徽标。
         原先是整卡片大留白的多行布局，压缩后 N 个工具 = N 行紧凑条目。 -->
    <div class="flex items-center gap-2 px-2.5 py-[7px]">
      <span class="text-[0.95rem] leading-none">{{ tone.emoji }}</span>
      <span class="min-w-0 truncate font-mono text-[0.8rem] text-white/90">{{ run.tool }}</span>

      <!-- 等待审批：行内小号允许/拒绝 -->
      <template v-if="run.status === 'pending'">
        <button
          class="inline-flex shrink-0 items-center rounded-md border border-emerald-400/40
            bg-emerald-400/15 px-2 py-[2px] text-[0.7rem] text-emerald-300 transition-all
            duration-200 hover:bg-emerald-400/25"
          :title="t('scriptEditor.agentTool.approvalHint')"
          @click="$emit('allow')"
        >
          {{ t("scriptEditor.agentTool.allow") }}
        </button>
        <button
          class="inline-flex shrink-0 items-center rounded-md border border-red-400/35 bg-red-400/12
            px-2 py-[2px] text-[0.7rem] text-red-300 transition-all duration-200
            hover:bg-red-400/25"
          @click="$emit('deny')"
        >
          {{ t("scriptEditor.agentTool.deny") }}
        </button>
      </template>

      <!-- 详情切换（有参数或结果才出现） -->
      <button
        v-if="hasArgs || run.output"
        class="ml-auto inline-flex shrink-0 items-center gap-1 text-[0.68rem] text-white/40
          transition-colors hover:text-white/80"
        @click="showDetails = !showDetails"
      >
        <span class="text-[0.6rem] leading-none">{{ showDetails ? "▾" : "▸" }}</span>
        {{
          showDetails
            ? t("scriptEditor.agentTool.hideDetails")
            : t("scriptEditor.agentTool.showDetails")
        }}
      </button>

      <!-- 状态徽标（running 时带转圈指示） -->
      <span
        class="inline-flex shrink-0 items-center gap-1 rounded-full border px-[7px] py-[1px]
          text-[0.68rem]"
        :class="statusMap[run.status].cls"
      >
        <span
          v-if="run.status === 'running'"
          class="h-2 w-2 shrink-0 animate-spin rounded-full border-[1.5px] border-current
            border-t-transparent"
        ></span>
        {{ statusMap[run.status].text }}
      </span>
    </div>

    <!-- 参数 / 结果展开区（保留原有内容，间距收紧） -->
    <div v-if="showDetails" class="space-y-1.5 overflow-hidden border-t border-white/8 px-2.5 py-2">
      <div v-if="hasArgs" class="rounded-lg border border-white/10 bg-black/25 px-2.5 py-1.5">
        <div class="mb-1 text-[0.68rem] text-white/40">{{ t("scriptEditor.agentTool.args") }}</div>
        <pre
          class="max-h-40 overflow-y-auto font-mono text-[0.72rem] whitespace-pre-wrap
            text-white/75"
          >{{ argsText }}</pre
        >
      </div>
      <div v-if="run.output" class="rounded-lg border border-white/10 bg-black/25 px-2.5 py-1.5">
        <div class="mb-1 text-[0.68rem] text-white/40">
          {{ t("scriptEditor.agentTool.result") }}
        </div>
        <pre
          class="max-h-52 overflow-y-auto font-mono text-[0.72rem] whitespace-pre-wrap
            text-white/75"
          >{{ run.output }}</pre
        >
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { computed, ref } from "vue";
  import { useI18n } from "vue-i18n";
  import type { ToolRun, ToolStatus } from "@/stores/modules/agent/state";

  const { t } = useI18n();
  const props = defineProps<{ run: ToolRun }>();
  defineEmits<{ (e: "allow"): void; (e: "deny"): void }>();

  const showDetails = ref(false);

  interface Tone {
    border: string;
    bg: string;
    emoji: string;
  }

  const TONES: Record<string, Tone> = {
    indigo: { border: "border-indigo-400/35", bg: "bg-indigo-400/8", emoji: "📖" },
    amber: { border: "border-amber-400/35", bg: "bg-amber-400/8", emoji: "💻" },
    emerald: { border: "border-emerald-400/35", bg: "bg-emerald-400/8", emoji: "📄" },
    red: { border: "border-red-400/35", bg: "bg-red-400/8", emoji: "🗑️" },
  };

  const toneOf = (tool: string): string => {
    if (tool === "execute_command") return "amber";
    if (tool === "read_skill" || tool === "list_skills") return "indigo";
    if (tool === "delete_file") return "red";
    return "emerald";
  };

  const tone = computed<Tone>(() => TONES[toneOf(props.run.tool)] ?? TONES.emerald);

  const statusMap: Record<ToolStatus, { text: string; cls: string }> = {
    running: {
      text: t("scriptEditor.agentTool.statusRunning"),
      cls: "text-amber-300 border-amber-300/30 bg-amber-300/10",
    },
    pending: {
      text: t("scriptEditor.agentTool.statusApproval"),
      cls: "text-blue-300 border-blue-300/30 bg-blue-300/10",
    },
    done: {
      text: t("scriptEditor.agentTool.statusDone"),
      cls: "text-emerald-300 border-emerald-300/30 bg-emerald-300/10",
    },
    error: {
      text: t("scriptEditor.agentTool.statusFailed"),
      cls: "text-red-300 border-red-300/30 bg-red-300/10",
    },
    denied: {
      text: t("scriptEditor.agentTool.statusDenied"),
      cls: "text-red-300 border-red-300/30 bg-red-300/10",
    },
  };

  const hasArgs = computed(() => props.run.args && Object.keys(props.run.args).length > 0);

  const argsText = computed(() => {
    try {
      return JSON.stringify(props.run.args, null, 2);
    } catch {
      return String(props.run.args);
    }
  });
</script>

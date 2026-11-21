<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 工具条 -->
    <div class="flex shrink-0 flex-wrap items-center gap-x-2 gap-y-1 px-3 pt-1 pb-2 text-[0.66rem]">
      <span v-if="ctx" class="text-white/40">
        {{ $t("game.director.context.systemChars", { n: ctx.stats.systemChars }) }}
      </span>
      <span v-if="ctx" class="text-white/40">
        {{ $t("game.director.context.historyChars", { n: ctx.stats.historyChars }) }}
      </span>
      <span v-if="ctx" class="text-white/40">
        {{ $t("game.director.context.messageCount", { n: ctx.stats.messageCount }) }}
      </span>
      <span v-if="ctx && ctx.stats.droppedMessages > 0" class="text-amber-300/80">
        {{ $t("game.director.context.dropped", { n: ctx.stats.droppedMessages }) }}
      </span>
      <button
        class="ml-auto inline-flex shrink-0 items-center gap-1 rounded-md border border-white/12 px-2 py-1 text-white/60 transition-colors hover:bg-white/8 hover:text-white/85 disabled:cursor-not-allowed disabled:opacity-40"
        :title="$t('game.director.context.refreshHint')"
        :disabled="store.loadingContext || !store.available"
        @click="refresh"
      >
        <RefreshCw :size="11" :class="{ 'animate-spin': store.loadingContext }" />
        {{
          store.loadingContext
            ? $t("game.director.context.refreshing")
            : $t("game.director.context.refresh")
        }}
      </button>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-3 py-1">
      <p v-if="!store.available" class="px-2 py-8 text-center text-[0.78rem] text-white/35">
        {{ $t("game.director.unavailable.title") }}
      </p>
      <p
        v-else-if="!ctx && store.loadingContext"
        class="px-2 py-8 text-center text-[0.78rem] text-white/35"
      >
        {{ $t("game.director.context.loading") }}
      </p>
      <p
        v-else-if="!ctx"
        class="px-2 py-8 text-center text-[0.78rem] leading-relaxed text-white/35"
      >
        {{ $t("game.director.context.noSnapshot") }}
      </p>

      <template v-else>
        <!-- system 分段 -->
        <section class="mb-3">
          <h4 class="mb-1.5 text-[0.72rem] font-semibold text-white/70">
            {{ $t("game.director.context.systemTitle") }}
          </h4>
          <div v-if="ctx.sections.length" class="space-y-1.5">
            <details
              v-for="sec in ctx.sections"
              :key="sec.key"
              class="director-fold rounded-lg border border-white/10 bg-black/20 px-2.5 py-1.5"
            >
              <summary
                class="flex cursor-pointer list-none items-center gap-1.5 text-[0.68rem] text-sky-300/80 select-none"
              >
                <component :is="sectionIcon(sec.key)" :size="12" class="shrink-0" />
                <span>{{ sec.title }}</span>
                <ChevronDown
                  :size="12"
                  class="director-fold-chevron ml-auto shrink-0 text-white/30 transition-transform duration-200"
                />
              </summary>
              <pre
                class="mt-1.5 max-h-48 overflow-y-auto font-mono text-[0.66rem] leading-relaxed break-all whitespace-pre-wrap text-white/65"
                >{{ sec.content }}</pre>
            </details>
          </div>
          <pre
            v-else
            class="max-h-72 overflow-y-auto rounded-lg border border-white/10 bg-black/20 p-2 font-mono text-[0.66rem] leading-relaxed break-all whitespace-pre-wrap text-white/65"
            >{{ ctx.systemPrompt }}</pre>
        </section>

        <!-- 消息序列 -->
        <section>
          <h4 class="mb-1.5 text-[0.72rem] font-semibold text-white/70">
            {{ $t("game.director.context.messagesTitle") }}
          </h4>
          <ul class="space-y-1.5">
            <li
              v-for="(msg, index) in ctx.messages"
              :key="index"
              class="rounded-lg border border-white/10 bg-white/4 px-2.5 py-1.5"
            >
              <div class="mb-1 flex items-center gap-1.5 text-[0.64rem]">
                <span class="text-white/30">{{ index }}</span>
                <span class="font-semibold" :class="roleClass(msg.role)">{{
                  roleLabel(msg.role)
                }}</span>
                <span v-if="msg.tool_call_id" class="truncate text-white/30">
                  {{ msg.tool_call_id }}
                </span>
              </div>
              <pre
                v-if="msg.content"
                class="max-h-48 overflow-y-auto font-mono text-[0.66rem] leading-relaxed break-all whitespace-pre-wrap text-white/70"
                >{{ msg.content }}</pre>
              <pre
                v-if="msg.tool_calls?.length"
                class="mt-1 max-h-40 overflow-y-auto rounded bg-black/30 p-1.5 font-mono text-[0.62rem] break-all whitespace-pre-wrap text-white/55"
                >{{ prettyJson(JSON.stringify(msg.tool_calls)) }}</pre>
            </li>
          </ul>
        </section>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, watch, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, MapPin, MessagesSquare, RefreshCw, Sparkles, Users } from "@lucide/vue";
import { useDirectorStore } from "@/stores/modules/director";

const { t } = useI18n();
const store = useDirectorStore();

const ctx = computed(() => store.context);

/** system 分段标题前的图标，按分段 key 取 */
const SECTION_ICONS: Record<string, Component> = {
  persona: Sparkles,
  scene: MapPin,
  roles: Users,
  recent: MessagesSquare,
};

const sectionIcon = (key: string) => SECTION_ICONS[key] ?? MessagesSquare;

/** 消息 role 的中文标签 */
const ROLE_LABELS: Record<string, string> = {
  system: "game.director.context.roleSystem",
  user: "game.director.context.roleUser",
  assistant: "game.director.context.roleAssistant",
  tool: "game.director.context.roleTool",
};

const roleLabel = (role: string) => (ROLE_LABELS[role] ? t(ROLE_LABELS[role]) : role);

const roleClass = (role: string) => {
  switch (role) {
    case "system":
      return "text-amber-300";
    case "user":
      return "text-sky-300";
    case "assistant":
      return "text-brand";
    case "tool":
      return "text-fuchsia-300";
    default:
      return "text-white/70";
  }
};

const prettyJson = (raw: string) => {
  if (!raw) return "—";
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
};

/** 打开面板默认展示上次实际请求的快照；「重新装配」按当前世界状态重算。 */
function refresh() {
  void store.loadContext(true);
}

onMounted(() => {
  if (!store.ready || !store.available) return;
  void store.loadContext(false);
});

// 面板先于状态加载挂载时，就绪后再补一次
watch(
  () => store.ready,
  (ready) => {
    if (ready && store.available && !store.context) void store.loadContext(false);
  },
);
</script>

<style scoped>
/* 折叠区展开 / 收起时高度平滑过渡；不支持 ::details-content 的运行时退化为原生开关 */
.director-fold {
  interpolate-size: allow-keywords;
}

.director-fold::details-content {
  block-size: 0;
  overflow: hidden;
  opacity: 0;
  transition:
    block-size 0.22s cubic-bezier(0.32, 0.72, 0, 1),
    opacity 0.22s cubic-bezier(0.32, 0.72, 0, 1),
    content-visibility 0.22s allow-discrete;
}

.director-fold[open]::details-content {
  block-size: auto;
  opacity: 1;
}

.director-fold[open] > summary .director-fold-chevron {
  transform: rotate(180deg);
}
</style>

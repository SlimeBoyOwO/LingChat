<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 消息区 -->
    <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto px-3 py-3" @scroll="onScroll">
      <!-- 服务不可用：后端未接入 / 没选模型 -->
      <div
        v-if="!canInteract"
        class="flex min-h-full flex-col items-center justify-center gap-2 px-4 text-center"
      >
        <Bot :size="26" class="text-white/20" />
        <p class="text-[0.82rem] text-white/55">
          {{
            store.ready
              ? $t("game.director.unavailable.noLlm")
              : $t("game.director.unavailable.title")
          }}
        </p>
        <p class="text-[0.7rem] leading-relaxed text-white/35">
          {{
            store.ready
              ? $t("game.director.unavailable.noLlmHint")
              : $t("game.director.unavailable.hint")
          }}
        </p>
        <p
          v-if="!store.ready && store.error"
          class="max-w-full truncate font-mono text-[0.62rem] text-white/25"
        >
          {{ store.error }}
        </p>
      </div>

      <template v-else>
        <p
          v-if="isEmpty"
          class="px-2 py-8 text-center text-[0.78rem] leading-relaxed text-white/35"
        >
          {{ $t("game.director.empty.chat") }}
        </p>

        <div class="space-y-3">
          <template v-for="(m, i) in store.messages" :key="`${m.id}-${i}`">
            <!-- 用户消息 -->
            <div v-if="m.role === 'user'" class="flex justify-end">
              <div
                class="max-w-[86%] rounded-2xl rounded-tr-sm border border-sky-400/30 bg-sky-400/10 px-3 py-2 text-[0.82rem] leading-relaxed wrap-break-word whitespace-pre-wrap text-white/90"
              >
                {{ m.content }}
              </div>
            </div>

            <!-- 导演回复 -->
            <div v-else-if="m.role === 'assistant'" class="flex flex-col gap-1.5">
              <details
                v-if="m.reasoning"
                class="rounded-lg border border-white/10 bg-white/4 px-2.5 py-1.5"
              >
                <summary class="cursor-pointer text-[0.68rem] text-white/40 select-none">
                  {{ $t("game.director.reasoning") }}
                </summary>
                <p
                  class="mt-1.5 text-[0.72rem] leading-relaxed wrap-break-word whitespace-pre-wrap text-white/45"
                >
                  {{ m.reasoning }}
                </p>
              </details>
              <div
                v-if="m.content"
                class="max-w-full rounded-2xl rounded-tl-sm border border-white/10 bg-white/8 px-3 py-2"
              >
                <MarkdownText :content="m.content" />
              </div>
            </div>
          </template>

          <!-- 本轮流式输出 -->
          <div v-if="store.streaming" class="flex flex-col gap-1.5">
            <details
              v-if="store.streaming.reasoning"
              open
              class="rounded-lg border border-white/10 bg-white/4 px-2.5 py-1.5"
            >
              <summary class="cursor-pointer text-[0.68rem] text-white/40 select-none">
                {{ $t("game.director.reasoning") }}
              </summary>
              <p
                class="mt-1.5 text-[0.72rem] leading-relaxed wrap-break-word whitespace-pre-wrap text-white/45"
              >
                {{ store.streaming.reasoning }}
              </p>
            </details>
            <div
              v-if="store.streaming.content"
              class="max-w-full rounded-2xl rounded-tl-sm border border-white/10 bg-white/8 px-3 py-2"
            >
              <MarkdownText :content="store.streaming.content" />
            </div>
            <p v-else-if="!store.streaming.reasoning" class="text-[0.74rem] text-white/35">
              {{ store.status || $t("game.director.thinking") }}
            </p>
          </div>

          <p
            v-if="store.error"
            class="flex items-start gap-1.5 rounded-lg border border-red-400/35 bg-red-500/10 px-3 py-2 text-[0.74rem] text-red-200"
          >
            <CircleExclamationPoint :size="13" class="mt-px shrink-0" />
            <span class="wrap-break-word">{{ store.error }}</span>
          </p>
        </div>
      </template>
    </div>

    <!-- 状态 / 清空 -->
    <div class="flex shrink-0 items-center gap-2 px-3 pb-1 text-[0.64rem]">
      <button
        class="text-white/30 transition-colors hover:text-red-300 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="!canInteract || store.running || isEmpty"
        @click="clearChat"
      >
        {{ $t("game.director.clear") }}
      </button>
      <span class="ml-auto truncate text-white/35">{{ store.status }}</span>
    </div>

    <!-- 输入区：服务未就绪时保留布局但禁用 -->
    <div class="shrink-0 px-3 pt-1 pb-3">
      <div
        class="flex items-end gap-2 rounded-xl border border-white/10 bg-black/25 px-3 py-2 transition-colors focus-within:border-sky-400/50"
        :class="{ 'opacity-50': !canInteract }"
      >
        <textarea
          v-model="draft"
          rows="1"
          :disabled="!canInteract"
          class="field-sizing-content max-h-32 flex-1 resize-none bg-transparent text-[0.82rem] leading-relaxed text-white outline-none placeholder:text-white/30"
          :placeholder="$t('game.director.inputPlaceholder')"
          @keydown.enter.exact.prevent="send"
          @compositionstart="composing = true"
          @compositionend="composing = false"
        ></textarea>
        <button
          v-if="store.running"
          class="shrink-0 rounded-lg border border-red-400/35 bg-red-400/12 px-3 py-1.5 text-[0.74rem] text-red-300 transition-colors hover:bg-red-400/25"
          @click="store.stop()"
        >
          {{ $t("game.director.stop") }}
        </button>
        <button
          v-else
          class="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-sky-400/40 bg-sky-400/12 px-3 py-1.5 text-[0.74rem] text-sky-200 transition-colors hover:bg-sky-400/24 disabled:opacity-40"
          :disabled="!canInteract || !draft.trim()"
          @click="send"
        >
          <span>{{ $t("game.director.send") }}</span>
          <Send :size="13" class="shrink-0" />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Bot, CircleExclamationPoint, Send } from "@lucide/vue";
import { MarkdownText } from "@/components/base";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import { useDirectorStore } from "@/stores/modules/director";
import { i18n } from "@/locales";

const store = useDirectorStore();
const dialogStore = useDialogStore();

const draft = ref("");
const composing = ref(false);

/** 服务就绪且空闲才能输入 / 清空 */
const canInteract = computed(() => store.ready && store.available);
const isEmpty = computed(() => store.messages.length === 0 && !store.streaming);

// ==================== 滚动策略 ====================

const NEAR_BOTTOM_THRESHOLD = 60;
const nearBottom = ref(true);
const scroller = ref<HTMLElement | null>(null);

function onScroll() {
  const el = scroller.value;
  if (!el) return;
  nearBottom.value = el.scrollTop + el.clientHeight >= el.scrollHeight - NEAR_BOTTOM_THRESHOLD;
}

function scrollToBottom(force = false) {
  nextTick(() => {
    requestAnimationFrame(() => {
      const el = scroller.value;
      if (!el || (!force && !nearBottom.value)) return;
      el.scrollTop = el.scrollHeight;
    });
  });
}

// 新消息或流式增量：贴底时跟随，用户上翻阅读时不打扰
watch(
  () => [store.messages.length, store.streaming?.content.length ?? 0],
  () => scrollToBottom(),
);

watch(
  () => store.ready,
  (ready) => {
    if (ready) scrollToBottom(true);
  },
);

// ==================== 发送 ====================

async function send() {
  const text = draft.value;
  if (composing.value || store.running || !text.trim()) return;
  draft.value = "";
  await store.send(text);
}

async function clearChat() {
  if (!(await dialogStore.confirm(i18n.global.t("game.director.clearConfirm")))) return;
  await store.clear();
}
</script>

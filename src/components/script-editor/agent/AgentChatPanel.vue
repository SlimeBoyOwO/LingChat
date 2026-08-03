<template>
  <div class="flex
    w-full
    min-h-0
    flex-1
    gap-4">
    <!-- 左栏：会话列表 -->
    <aside class="flex
      w-[230px]
      min-h-0
      shrink-0
      flex-col
      gap-3">
      <button
        class="inline-flex
          items-center
          justify-center
          gap-1
          rounded-xl
          border
          border-brand/45
          bg-brand/14
          px-3
          py-2
          text-[0.82rem]
          text-brand
          transition-all
          duration-200
          hover:bg-brand/24
          disabled:opacity-50"
        :disabled="store.loading"
        @click="store.createConversation()"
      >
        <span class="text-[1rem]
          leading-none">＋</span> 新建对话
      </button>

      <div class="flex
        min-h-0
        flex-1
        flex-col
        gap-1.5
        overflow-y-auto
        pr-1">
        <button
          v-for="c in store.conversations"
          :key="c.id"
          class="group
            rounded-[10px]
            border
            px-3
            py-2.5
            text-left
            transition-all
            duration-200"
          :class="
            c.id === store.currentId
              ? `border-brand/60
                bg-brand/12`
              : `border-white/10
                bg-white/6
                hover:border-brand/40
                hover:bg-white/10`
          "
          @click="store.switchConversation(c.id)"
        >
          <div class="flex
            items-center
            justify-between
            gap-2">
            <span class="truncate
              text-[0.8rem]
              text-white/85">{{
              c.title || `对话 ${c.id}`
            }}</span>
            <span
              class="opacity-0
                transition-opacity
                group-hover:opacity-100"
              :title="'删除此对话'"
              @click.stop="removeConversation(c)"
            >
              <Icon
                icon="close"
                :size="13"
                class="cursor-pointer
                  text-white/50
                  hover:text-red-300"
              />
            </span>
          </div>
          <div
            v-if="c.scriptKey"
            class="mt-1
              truncate
              font-mono
              text-[0.66rem]
              text-brand/70"
          >
            📕 {{ c.scriptKey }}
          </div>
        </button>
      </div>

      <button
        class="text-[0.72rem]
          text-white/40
          transition-colors
          hover:text-white/70"
        @click="store.clearConversation()"
      >
        清空当前对话
      </button>
    </aside>

    <!-- 右栏：聊天 -->
    <div
      class="flex
        min-w-0
        min-h-0
        flex-1
        flex-col
        overflow-hidden
        rounded-xl
        border
        border-white/10
        bg-white/4"
    >
      <!-- 消息区 -->
      <div
        ref="scroller"
        class="flex
          min-h-0
          flex-1
          flex-col
          gap-3
          overflow-y-auto
          px-4
          py-4"
      >
        <div
          v-if="store.loading"
          class="py-10
            text-center
            text-[0.82rem]
            text-white/40"
        >
          加载中…
        </div>
        <div
          v-else-if="!store.currentId"
          class="py-10
            text-center
            text-[0.82rem]
            text-white/40"
        >
          还没有对话，点左侧「新建对话」开始
        </div>

        <template v-else>
          <template
            v-for="item in store.items"
            :key="item.id"
          >
            <!-- 用户消息 -->
            <div
              v-if="item.role === 'user'"
              class="flex
                justify-end"
            >
              <div
                class="max-w-[76%]
                  whitespace-pre-wrap
                  break-words
                  rounded-2xl
                  rounded-tr-sm
                  border
                  border-brand/40
                  bg-brand/12
                  px-3.5
                  py-2.5
                  text-[0.86rem]
                  leading-relaxed
                  text-white/90"
              >
                {{ item.content }}
              </div>
            </div>

            <!-- assistant 回复 -->
            <div
              v-else
              class="flex
                flex-col
                gap-2"
            >
              <div
                v-for="(round, i) in item.rounds"
                :key="i"
                class="flex
                  flex-col
                  gap-2"
              >
                <div
                  v-if="round.content"
                  class="max-w-[92%]
                    whitespace-pre-wrap
                    break-words
                    rounded-2xl
                    rounded-tl-sm
                    border
                    border-white/10
                    bg-white/8
                    px-3.5
                    py-2.5
                    text-[0.86rem]
                    leading-relaxed
                    text-white/85"
                >
                  {{ round.content }}
                </div>
                <AgentToolCard
                  v-for="run in round.toolRuns"
                  :key="run.callId"
                  :run="run"
                  class="max-w-[92%]"
                  @allow="run.requestId && store.resolveApproval(run.requestId, true)"
                  @deny="run.requestId && store.resolveApproval(run.requestId, false)"
                />
              </div>
              <div
                v-if="item.error"
                class="text-[0.78rem]
                  text-red-300"
              >
                ⚠ {{ item.error }}
              </div>
              <div
                v-if="item.streaming && item.rounds.length === 0"
                class="text-[0.78rem]
                  text-white/40"
              >
                思考中…
              </div>
            </div>
          </template>
        </template>
      </div>

      <!-- 状态行 -->
      <div
        v-if="store.status || store.lastUsage"
        class="flex
          items-center
          justify-between
          px-4
          pb-1
          text-[0.7rem]
          text-white/40"
      >
        <span class="truncate">{{ store.status }}</span>
        <span
          v-if="store.lastUsage"
          class="shrink-0
            font-mono"
        >
          {{ store.lastUsage.total_tokens }} tokens
        </span>
      </div>

      <!-- 输入区 -->
      <div class="border-t
        border-white/10
        px-3
        py-2.5">
        <div
          class="flex
            items-end
            gap-2
            rounded-xl
            border
            border-white/10
            bg-black/25
            px-3
            py-2
            focus-within:border-brand/50"
        >
          <textarea
            ref="inputEl"
            v-model="draft"
            rows="1"
            class="max-h-32
              flex-1
              resize-none
              bg-transparent
              text-[0.86rem]
              text-white
              outline-none
              placeholder:text-white/35"
            placeholder="让 AI 助手写剧本、改文件、执行命令…（Enter 发送，Shift+Enter 换行）"
            :disabled="store.sending"
            @keydown.enter.exact.prevent="send"
            @compositionstart="composing = true"
            @compositionend="composing = false"
          ></textarea>
          <button
            v-if="store.streaming"
            class="inline-flex
              shrink-0
              items-center
              gap-1
              rounded-lg
              border
              border-red-400/35
              bg-red-400/12
              px-3
              py-1.5
              text-[0.78rem]
              text-red-300
              transition-colors
              hover:bg-red-400/25"
            @click="store.cancel()"
          >
            停止
          </button>
          <button
            v-else
            class="inline-flex
              shrink-0
              items-center
              gap-1
              rounded-lg
              border
              border-brand/45
              bg-brand/14
              px-3
              py-1.5
              text-[0.78rem]
              text-brand
              transition-colors
              hover:bg-brand/24
              disabled:opacity-50"
            :disabled="!draft.trim() || store.sending"
            @click="send"
          >
            发送
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue'
import { Icon } from '@/components/base'
import { useAgentStore } from '@/stores/modules/agent'
import AgentToolCard from './AgentToolCard.vue'
import type { ConversationInfo } from '@/api/services/agent'

const store = useAgentStore()

const draft = ref('')
const composing = ref(false)
const scroller = ref<HTMLElement | null>(null)
const inputEl = ref<HTMLTextAreaElement | null>(null)

function scrollToBottom() {
  nextTick(() => {
    if (scroller.value) scroller.value.scrollTop = scroller.value.scrollHeight
  })
}

// 每次事件/切换会话后滚到底
watch(
  () => [store.version, store.currentId],
  () => scrollToBottom(),
)

watch(store.conversations, () => scrollToBottom())

onMounted(() => {
  void store.initForEditor()
})

async function send() {
  const text = draft.value
  if (composing.value || store.streaming || !text.trim()) return
  draft.value = ''
  await store.sendMessage(text)
}

async function removeConversation(c: ConversationInfo) {
  if (!window.confirm(`确定删除对话「${c.title || `对话 ${c.id}`}」及其全部消息吗？`)) return
  await store.deleteConversation(c.id)
}
</script>

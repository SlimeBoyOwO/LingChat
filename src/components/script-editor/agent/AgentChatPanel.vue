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
          leading-none">＋</span>
        {{ t('scriptEditor.agentChat.newConversation') }}
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
          @click="editingTitleId !== c.id && store.switchConversation(c.id)"
        >
          <!-- 编辑态：inline 输入（回车保存 / Esc 取消 / 失焦保存） -->
          <input
            v-if="editingTitleId === c.id"
            v-model="titleDraft"
            ref="renameInputEl"
            class="w-full
              rounded-md
              border
              border-brand/50
              bg-[rgba(0,0,0,0.45)]
              px-2
              py-1
              text-[0.8rem]
              text-white/90
              outline-none"
            :placeholder="t('scriptEditor.agentChat.renamePlaceholder')"
            maxlength="60"
            @click.stop
            @keydown.enter.prevent="commitRename(c)"
            @keydown.esc.prevent="cancelRename"
            @blur="commitRename(c)"
          />
          <template v-else>
            <div class="flex
              items-center
              justify-between
              gap-2">
              <span class="truncate
                text-[0.8rem]
                text-white/85">{{
                c.title || t('scriptEditor.agentChat.conversationTitle', { id: c.id })
              }}</span>
              <span
                class="opacity-0
                  transition-opacity
                  group-hover:opacity-100
                  inline-flex
                  items-center
                  gap-1"
              >
                <Icon
                  icon="edit"
                  :size="13"
                  class="cursor-pointer
                    text-white/50
                    hover:text-brand"
                  :title="t('scriptEditor.agentChat.rename')"
                  @click.stop="startRename(c)"
                />
                <Icon
                  icon="close"
                  :size="13"
                  class="cursor-pointer
                    text-white/50
                    hover:text-red-300"
                  :title="t('scriptEditor.agentChat.deleteConversation')"
                  @click.stop="removeConversation(c)"
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
          </template>
        </button>
      </div>

      <button
        class="text-[0.72rem]
          text-white/40
          transition-colors
          hover:text-white/70"
        @click="clearConversation"
      >
        {{ t('scriptEditor.agentChat.clearConversation') }}
      </button>

      <!-- Token 用量（窗口左下角；折叠卡片） -->
      <div class="shrink-0
        rounded-xl
        border
        border-white/10
        bg-white/5
        px-3
        py-2.5">
        <button
          class="flex
            w-full
            items-center
            justify-between
            gap-2
            text-left"
          :title="
            usageOpen
              ? t('scriptEditor.agentChat.collapseUsage')
              : t('scriptEditor.agentChat.expandUsage')
          "
          @click="usageOpen = !usageOpen"
        >
          <span class="inline-flex
            items-center
            gap-1.5
            text-[0.72rem]
            text-white/50">
            <Icon
              icon="advance"
              :size="13"
              class="text-brand"
            />
            {{ t('scriptEditor.agentChat.tokenUsage') }}
          </span>
          <span class="font-mono
            text-[0.78rem]
            text-brand">{{
            store.totalTokens.toLocaleString()
          }}</span>
          <span class="text-[0.6rem]
            text-white/30">{{ usageOpen ? '▾' : '▸' }}</span>
        </button>

        <div
          v-if="usageOpen"
          class="mt-2
            flex
            flex-col
            gap-1.5
            border-t
            border-white/10
            pt-2"
        >
          <template v-if="store.lastUsage">
            <div class="grid
              grid-cols-3
              gap-1
              text-center">
              <div class="rounded-md
                bg-white/5
                py-1">
                <div class="text-[0.6rem]
                  text-white/40">
                  {{ t('scriptEditor.agentChat.input') }}
                </div>
                <div class="font-mono
                  text-[0.72rem]
                  text-white/85">
                  {{ store.lastUsage.prompt_tokens.toLocaleString() }}
                </div>
              </div>
              <div class="rounded-md
                bg-white/5
                py-1">
                <div class="text-[0.6rem]
                  text-white/40">
                  {{ t('scriptEditor.agentChat.output') }}
                </div>
                <div class="font-mono
                  text-[0.72rem]
                  text-white/85">
                  {{ store.lastUsage.completion_tokens.toLocaleString() }}
                </div>
              </div>
              <div class="rounded-md
                bg-white/5
                py-1">
                <div class="text-[0.6rem]
                  text-white/40">
                  {{ t('scriptEditor.agentChat.currentRound') }}
                </div>
                <div class="font-mono
                  text-[0.72rem]
                  text-brand">
                  {{ store.lastUsage.total_tokens.toLocaleString() }}
                </div>
              </div>
            </div>
            <div
              v-if="store.lastUsage.cached_tokens > 0 && store.lastUsage.prompt_tokens > 0"
              class="text-[0.62rem]
                text-brand/80"
            >
              {{
                t('scriptEditor.agentChat.cacheHit', {
                  cached: store.lastUsage.cached_tokens.toLocaleString(),
                  prompt: store.lastUsage.prompt_tokens.toLocaleString(),
                  percent: Math.round(
                    (store.lastUsage.cached_tokens / store.lastUsage.prompt_tokens) * 100,
                  ),
                })
              }}
            </div>
            <div class="text-[0.62rem]
              text-white/35">
              {{
                t('scriptEditor.agentChat.totalTokens', {
                  count: store.totalTokens.toLocaleString(),
                })
              }}
            </div>
          </template>
          <p
            v-else
            class="text-[0.68rem]
              text-white/35"
          >
            {{ t('scriptEditor.agentChat.usageEmpty') }}
          </p>
        </div>
      </div>
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
        @scroll="onScroll"
      >
        <div
          v-if="store.loading"
          class="py-10
            text-center
            text-[0.82rem]
            text-white/40"
        >
          {{ t('scriptEditor.agentChat.loading') }}
        </div>
        <div
          v-else-if="!store.currentId"
          class="py-10
            text-center
            text-[0.82rem]
            text-white/40"
        >
          {{ t('scriptEditor.agentChat.empty') }}
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
                flex-col
                items-end
                gap-1"
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
              <!-- 回溯：常驻显示（低透明，hover 加深），点击把文字覆盖进输入框 -->
              <button
                class="inline-flex
                  items-center
                  gap-1
                  rounded-full
                  px-2
                  py-0.5
                  text-[0.7rem]
                  text-white/35
                  transition-colors
                  duration-200
                  hover:text-brand"
                :title="t('scriptEditor.agentChat.quote')"
                @click="quoteMessage(item)"
              >
                <span class="text-[0.6rem]
                  leading-none">↩</span>
                {{ t('scriptEditor.agentChat.quote') }}
              </button>
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
                <!-- 思考/规划块：有思考链，或该轮以工具调用结尾（正文是工具前叙述）。
                     streaming 时 pill 显示转圈 + 「思考中」高亮。 -->
                <AgentThinkingBlock
                  v-if="thinkingText(round)"
                  :text="thinkingText(round)"
                  :streaming="item.streaming"
                />
                <!-- 普通回复气泡：独立判断，与思考块并存显示。
                     开启思考模式时最终答复轮同时携带 reasoning + content，
                     若用 v-else-if 会吞掉正文；工具轮（含叙述）也在此排除，
                     其正文已并入上方思考块。 -->
                <div
                  v-if="round.content && round.toolRuns.length === 0"
                  class="max-w-[92%]
                    rounded-2xl
                    rounded-tl-sm
                    border
                    border-white/10
                    bg-white/8
                    px-3.5
                    py-2.5"
                >
                  <MarkdownText :content="round.content" />
                </div>
                <!-- 工具调用折叠组：一轮的多次调用合并为一个容器，默认收起防刷屏；
                     有等待审批（pending）时自动展开，保证允许/拒绝按钮可见 -->
                <div
                  v-if="round.toolRuns.length > 0"
                  class="w-full
                    max-w-[92%]
                    overflow-hidden
                    rounded-[10px]
                    border
                    border-white/10
                    bg-white/5"
                >
                  <button
                    class="flex
                      w-full
                      items-center
                      gap-2
                      px-3
                      py-2
                      text-left
                      transition-colors
                      hover:bg-white/8"
                    :aria-expanded="toolGroupOpen(item.id, i, round)"
                    @click="toggleToolGroup(item.id, i)"
                  >
                    <span class="text-[0.9rem]
                      leading-none">🔧</span>
                    <span class="text-[0.78rem]
                      text-white/75">{{
                      t('scriptEditor.agentTool.groupTitle', {
                        count: round.toolRuns.length,
                      })
                    }}</span>
                    <span class="ml-auto
                      inline-flex
                      items-center
                      gap-1.5">
                      <span
                        v-if="toolGroupSummary(round)"
                        class="text-[0.68rem]
                          text-white/40"
                        >{{ toolGroupSummary(round) }}</span
                      >
                      <span class="text-[0.6rem]
                        text-white/35">{{
                        toolGroupOpen(item.id, i, round) ? '▾' : '▸'
                      }}</span>
                    </span>
                  </button>
                  <div
                    v-if="toolGroupOpen(item.id, i, round)"
                    class="flex
                      flex-col
                      gap-1
                      border-t
                      border-white/10
                      p-1.5"
                  >
                    <AgentToolCard
                      v-for="run in round.toolRuns"
                      :key="run.callId"
                      :run="run"
                      @allow="approveRun(run, item.id, i, true)"
                      @deny="approveRun(run, item.id, i, false)"
                    />
                  </div>
                </div>
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
                {{ t('scriptEditor.agentChat.thinking') }}
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
            class="max-h-40
              flex-1
              resize-y
              bg-transparent
              text-[0.86rem]
              leading-relaxed
              text-white
              outline-none
              placeholder:text-white/35
              [field-sizing:content]"
            :placeholder="t('scriptEditor.agentChat.placeholder')"
            :disabled="store.sending"
            @input="autoResizeInput"
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
            {{ t('scriptEditor.agentChat.stop') }}
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
            {{ t('scriptEditor.agentChat.send') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Icon } from '@/components/base'
import { useDialogStore } from '@/stores/modules/ui/dialog'
import { useAgentStore } from '@/stores/modules/agent'
import AgentThinkingBlock from './AgentThinkingBlock.vue'
import AgentToolCard from './AgentToolCard.vue'
import MarkdownText from './MarkdownText.vue'
import type { ConversationInfo } from '@/api/services/agent'
import type { ChatItem, ChatRound, ToolRun } from '@/stores/modules/agent/state'

const { t } = useI18n()
const store = useAgentStore()
const dialogStore = useDialogStore()

const draft = ref('')
const composing = ref(false)

/**
 * 一轮的「思考/规划」展示文本（折叠思考块内容）：
 * - 该轮携带思考链（thinking 模式开启）→ 显示思考链；
 * - 该轮以工具调用结尾 → 正文是工具前的叙述，一并放入思考块；
 * - 纯文本且无工具调用（最终答复）→ 返回空，正文走独立气泡（见模板）。
 */
function thinkingText(round: ChatRound): string {
  const parts = [round.reasoning, round.toolRuns.length > 0 ? round.content : null].filter(
    (s): s is string => !!s,
  )
  return parts.join('\n\n')
}

// ==================== 工具调用折叠组 ====================

/** 手动展开过的工具调用组，key = 「消息 id:轮次」。会话切换/重建后旧 key 自然失效。 */
const openToolGroups = ref<Set<string>>(new Set())

function toolGroupKey(itemId: string, roundIdx: number): string {
  return `${itemId}:${roundIdx}`
}

/** 组是否展开：手动展开过（含审批交互过的组），或组内有等待审批（pending）的工具。 */
function toolGroupOpen(itemId: string, roundIdx: number, round: ChatRound): boolean {
  return (
    openToolGroups.value.has(toolGroupKey(itemId, roundIdx)) ||
    round.toolRuns.some((r) => r.status === 'pending')
  )
}

function toggleToolGroup(itemId: string, roundIdx: number) {
  const key = toolGroupKey(itemId, roundIdx)
  if (openToolGroups.value.has(key)) {
    openToolGroups.value.delete(key)
  } else {
    openToolGroups.value.add(key)
  }
}

/** 折叠态头部状态摘要：pending > running > 部分失败 > 全部完成。 */
function toolGroupSummary(round: ChatRound): string {
  const runs = round.toolRuns
  if (runs.some((r) => r.status === 'pending')) {
    return t('scriptEditor.agentTool.groupPending')
  }
  if (runs.some((r) => r.status === 'running')) {
    return t('scriptEditor.agentTool.groupRunning')
  }
  if (runs.some((r) => r.status === 'error' || r.status === 'denied')) {
    return t('scriptEditor.agentTool.groupFailed')
  }
  return t('scriptEditor.agentTool.groupDone')
}

/** 审批工具调用；交互过的组保持展开，方便观察执行结果。 */
async function approveRun(run: ToolRun, itemId: string, roundIdx: number, allowed: boolean) {
  openToolGroups.value.add(toolGroupKey(itemId, roundIdx))
  if (run.requestId) await store.resolveApproval(run.requestId, allowed)
}

/**
 * 回溯用户消息：把消息文字覆盖到输入框并聚焦、光标置于末尾，随后删除该消息
 * 及其后所有回复（撤回重发语义）——重发即新的一轮，历史里不留残缺回合。
 */
async function quoteMessage(item: ChatItem) {
  draft.value = item.content
  void nextTick(() => {
    const el = inputEl.value
    if (!el) return
    el.focus()
    el.setSelectionRange(el.value.length, el.value.length)
  })
  try {
    await store.rewindMessage(item)
  } catch (err) {
    console.warn('[Agent] 回溯删除消息失败:', err)
  }
}

/** 左下角 Token 用量卡片是否展开明细。 */
const usageOpen = ref(false)
const scroller = ref<HTMLElement | null>(null)
const inputEl = ref<HTMLTextAreaElement | null>(null)

/**
 * 输入框随内容自动增高。
 * 优先用原生 `field-sizing: content`（WebView2 基于新版 Chromium，原生支持，且
 * 与 `resize-y` 手动拖动天然兼容：拖出的内联高度优先于内容尺寸）；不支持的旧
 * 内核退回 JS 量高。两个路径都不写死高度上限（max-h-40 由 CSS 封顶，超出滚动）。
 */
const MAX_INPUT_HEIGHT = 160
const fieldSizingSupported = typeof CSS !== 'undefined' && CSS.supports('field-sizing', 'content')

function autoResizeInput() {
  if (fieldSizingSupported) return
  const el = inputEl.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, MAX_INPUT_HEIGHT)}px`
}

// ==================== 滚动策略 ====================

/** 距底部多少像素内视为「贴底」：贴底时流式输出自动跟随，上翻阅读时不打扰。 */
const NEAR_BOTTOM_THRESHOLD = 80
const nearBottom = ref(true)

function onScroll() {
  const el = scroller.value
  if (!el) return
  nearBottom.value = el.scrollTop + el.clientHeight >= el.scrollHeight - NEAR_BOTTOM_THRESHOLD
}

/**
 * 滚动到对话底部。
 * - force：进入面板 / 切换会话 / 历史加载完成等整段内容更替时无条件到底；
 * - 非 force（流式事件）：仅贴底时跟随，用户上翻阅读时不再回弹。
 * nextTick 等 DOM 更新后量高度，rAF 再等一帧，避开 tab 入场动画与
 * loading → 消息列表切换的布局时机。
 */
function scrollToBottom(force = false) {
  nextTick(() => {
    requestAnimationFrame(() => {
      const el = scroller.value
      if (!el) return
      if (!force && !nearBottom.value) return
      el.scrollTop = el.scrollHeight
    })
  })
}

// 流式事件（每事件 version++）：贴底时跟随，用户上翻时不打扰
watch(
  () => store.version,
  () => scrollToBottom(),
)

// 切换会话：整段内容更替，无条件滚到底
watch(
  () => store.currentId,
  () => scrollToBottom(true),
)

// 历史加载完成（loading → 消息列表切换）后滚到底
watch(
  () => store.loading,
  (v) => {
    if (!v) scrollToBottom(true)
  },
)

// 会话消息整体替换（getAgentMessages 重建 / 清空）后滚到底
watch(
  () => store.items,
  () => scrollToBottom(true),
)

onMounted(() => {
  void store.initForEditor()
})

async function send() {
  const text = draft.value
  if (composing.value || store.streaming || !text.trim()) return
  draft.value = ''
  // 清空后把高度收回去（field-sizing 原生会自动收，这里给 JS 兜底路径）
  void nextTick(autoResizeInput)
  await store.sendMessage(text)
}

async function removeConversation(c: ConversationInfo) {
  const ok = await dialogStore.confirm(
    t('scriptEditor.agentChat.deleteConfirm', {
      title: c.title || t('scriptEditor.agentChat.conversationTitle', { id: c.id }),
    }),
  )
  if (!ok) return
  await store.deleteConversation(c.id)
}

// ---- 重命名（inline 编辑） ----
/** 正在编辑标题的会话 id（null = 无编辑态）。 */
const editingTitleId = ref<number | null>(null)
/** 编辑中的标题草稿。 */
const titleDraft = ref('')
/** 编辑态输入框的 DOM 引用（聚焦/全选用）。 */
const renameInputEl = useTemplateRef<HTMLInputElement>('renameInputEl')
/** 标记本次编辑是否已取消：Esc 先置 true，随后触发的 blur 不再保存。 */
let renameCancelled = false

function startRename(c: ConversationInfo) {
  editingTitleId.value = c.id
  titleDraft.value = c.title ?? ''
  renameCancelled = false
  // 输入框挂载后聚焦并全选，便于直接覆盖输入
  void nextTick(() => {
    renameInputEl.value?.focus()
    renameInputEl.value?.select()
  })
}

/** 保存：空标题不退出编辑态（避免「点了没反应」的无声失败），失败时回退编辑态。 */
async function commitRename(c: ConversationInfo) {
  if (editingTitleId.value !== c.id) return
  if (renameCancelled) {
    editingTitleId.value = null
    return
  }
  const trimmed = titleDraft.value.trim()
  if (!trimmed) return
  editingTitleId.value = null
  try {
    await store.renameConversation(c.id, trimmed)
  } catch (err) {
    console.warn('[Agent] 重命名会话失败:', err)
    editingTitleId.value = c.id
    void nextTick(() => renameInputEl.value?.focus())
  }
}

function cancelRename() {
  renameCancelled = true
  editingTitleId.value = null
}

/** 清空当前对话为危险操作，先弹确认框（移动端容易误触）。 */
async function clearConversation() {
  if (!(await dialogStore.confirm(t('scriptEditor.agentChat.clearConfirm')))) return
  await store.clearConversation()
}
</script>

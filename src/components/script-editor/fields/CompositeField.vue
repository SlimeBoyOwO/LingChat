<template>
  <div class="rounded-xl
    border
    border-white/10
    bg-black/15
    p-3">
    <!-- ============ choices 的选项列表 ============ -->
    <template v-if="field.kind === 'choice_options'">
      <div
        v-for="(opt, i) in rows"
        :key="i"
        class="mb-2
          rounded-lg
          bg-white/6
          p-2.5
          last:mb-0"
      >
        <div class="mb-1.5
          flex
          items-center
          gap-2">
          <span class="text-xs
            text-white/40">{{ i + 1 }}</span>
          <input
            class="w-full
              min-w-0
              border
              border-white/[0.1]
              rounded-md
              bg-black/[0.25]
              px-2
              py-1.5
              text-xs
              text-white
              transition-all
              focus:outline-none
              focus:border-[var(--accent-color)]"
            :placeholder="t('scriptEditor.composite.fallbackLabel')"
            :value="str(opt.text)"
            @change="(e) => patch(i, 'text', val(e))"
          />
          <button
            class="shrink-0
              rounded-md
              px-1.5
              py-1
              text-xs
              text-white/[0.35]
              transition-all
              hover:text-[#fca5a5]
              hover:bg-[rgba(248,113,113,0.15)]"
            :title="t('scriptEditor.composite.deleteOption')"
            @click="removeRow(i)"
          >
            ✕
          </button>
        </div>

        <!-- 选项级条件（引擎支持 options[].condition）：默认收起，点「＋ 条件」才展开。
             说明在卡片底部统一给出，这里隐藏编辑器自带的顶部行 -->
        <div
          v-if="conditionOpen(i)"
          class="mt-1
            flex
            items-center
            gap-2
            pl-6"
        >
          <span class="shrink-0
            text-xs
            text-white/40">{{
            t('scriptEditor.composite.condition')
          }}</span>
          <ConditionEditor
            class="flex-1
              min-w-0"
            :model-value="str(opt.condition)"
            :variables="store.variables"
            :hint="''"
            @update:model-value="(v: string) => patch(i, 'condition', v)"
          />
          <button
            class="shrink-0
              rounded-md
              px-1.5
              py-1
              text-xs
              text-white/[0.35]
              transition-all
              hover:text-[#fca5a5]
              hover:bg-[rgba(248,113,113,0.15)]"
            :title="t('scriptEditor.composite.removeCondition')"
            @click="closeCondition(i)"
          >
            ✕
          </button>
        </div>
        <div
          v-if="conditionOpen(i)"
          class="mt-1
            flex
            items-center
            gap-2
            pl-6"
        >
          <span class="shrink-0
            text-xs
            text-white/40">{{
            t('scriptEditor.composite.disabledHint')
          }}</span>
          <input
            class="flex-1
              min-w-0
              border
              border-white/[0.1]
              rounded-md
              bg-black/[0.25]
              px-2
              py-1.5
              text-xs
              text-white
              transition-all
              focus:outline-none
              focus:border-[var(--accent-color)]"
            :placeholder="t('scriptEditor.composite.lockHintPlaceholder')"
            :value="str(opt.lock_hint)"
            @change="(e) => patch(i, 'lock_hint', val(e))"
          />
        </div>

        <!-- 选项动作：每类动作独立按钮添加，不用下拉 -->
        <div
          v-for="(act, ai) in actions(opt)"
          :key="ai"
          class="mt-1
            flex
            items-center
            gap-2
            pl-6"
        >
          <span class="shrink-0
            text-xs
            text-white/40">{{ actionLabel(str(act.type)) }}</span>
          <VariableEditor
            v-if="act.type === 'set_var' && !legacySetVar(act)"
            class="flex-1
              min-w-0"
            :model-value="str(act.content)"
            :variables="store.variables"
            :hint="''"
            @update:model-value="(v: string) => patchAction(i, ai, 'content', v)"
          />
          <div
            v-else-if="legacySetVar(act)"
            class="flex-1
              min-w-0
              rounded-md
              border
              border-yellow-300/25
              bg-yellow-300/10
              px-2
              py-1.5"
          >
            <p class="text-xs
              text-yellow-200">
              {{ t('scriptEditor.composite.legacyNotice') }}
            </p>
            <button
              class="mt-1
                text-xs
                text-brand
                hover:underline"
              @click="convertLegacy(i, ai)"
            >
              {{ t('scriptEditor.composite.convert') }}
            </button>
          </div>
          <input
            v-else
            class="flex-1
              min-w-0
              border
              border-white/[0.1]
              rounded-md
              bg-black/[0.25]
              px-2
              py-1.5
              text-xs
              text-white
              transition-all
              focus:outline-none
              focus:border-[var(--accent-color)]"
            :placeholder="actionPlaceholder(str(act.type))"
            :value="str(act.content)"
            @change="(e) => patchAction(i, ai, 'content', val(e))"
          />
          <button
            class="shrink-0
              rounded-md
              px-1.5
              py-1
              text-xs
              text-white/[0.35]
              transition-all
              hover:text-[#fca5a5]
              hover:bg-[rgba(248,113,113,0.15)]"
            :title="t('scriptEditor.composite.deleteAction')"
            @click="removeAction(i, ai)"
          >
            ✕
          </button>
        </div>

        <!-- 动作区：每个可用动作一行 = 高亮概念名 + 短说明 + 右侧添加。
             与底部说明合并，行距拉开，避免按钮横排拥挤 -->
        <div class="mt-1.5
          ml-6
          flex
          flex-col">
          <div
            class="flex
              items-center
              gap-2.5
              rounded-md
              px-1.5
              py-1.5
              transition-colors
              hover:bg-white/[0.04]"
          >
            <span class="w-1.5
              h-1.5
              shrink-0
              rounded-full
              bg-brand"></span>
            <span class="shrink-0
              text-xs
              font-semibold
              text-brand">{{
              t('scriptEditor.composite.addPlayerLine')
            }}</span>
            <span class="min-w-0
              flex-1
              text-xs
              leading-snug
              text-white/40">{{
              t('scriptEditor.composite.addLineDesc')
            }}</span>
            <button
              v-if="!hasAddLine(i)"
              class="shrink-0
                rounded-full
                border
                border-brand/40
                bg-brand/10
                px-2.5
                py-0.5
                text-xs
                text-brand
                transition-all
                hover:bg-brand/20"
              @click="addAction(i, 'add_line')"
            >
              ＋ {{ t('scriptEditor.composite.addBtn') }}
            </button>
            <span
              v-else
              class="shrink-0
                text-xs
                text-green-400/90"
              >{{ t('scriptEditor.composite.added') }}</span
            >
          </div>
          <div
            class="flex
              items-center
              gap-2.5
              rounded-md
              px-1.5
              py-1.5
              transition-colors
              hover:bg-white/[0.04]"
          >
            <span class="w-1.5
              h-1.5
              shrink-0
              rounded-full
              bg-brand"></span>
            <span class="shrink-0
              text-xs
              font-semibold
              text-brand">{{
              t('scriptEditor.composite.addVariable')
            }}</span>
            <span class="min-w-0
              flex-1
              text-xs
              leading-snug
              text-white/40">{{
              t('scriptEditor.composite.addVarDesc')
            }}</span>
            <button
              class="shrink-0
                rounded-full
                border
                border-brand/40
                bg-brand/10
                px-2.5
                py-0.5
                text-xs
                text-brand
                transition-all
                hover:bg-brand/20"
              @click="addAction(i, 'set_var')"
            >
              ＋ {{ t('scriptEditor.composite.addBtn') }}
            </button>
          </div>
          <div
            class="flex
              items-center
              gap-2.5
              rounded-md
              px-1.5
              py-1.5
              transition-colors
              hover:bg-white/[0.04]"
          >
            <span class="w-1.5
              h-1.5
              shrink-0
              rounded-full
              bg-brand"></span>
            <span class="shrink-0
              text-xs
              font-semibold
              text-brand">{{
              t('scriptEditor.composite.addCondition')
            }}</span>
            <span class="min-w-0
              flex-1
              text-xs
              leading-snug
              text-white/40">{{
              t('scriptEditor.composite.addConditionDesc')
            }}</span>
            <button
              v-if="!conditionOpen(i)"
              class="shrink-0
                rounded-full
                border
                border-brand/40
                bg-brand/10
                px-2.5
                py-0.5
                text-xs
                text-brand
                transition-all
                hover:bg-brand/20"
              @click="openCondition(i)"
            >
              ＋ {{ t('scriptEditor.composite.addBtn') }}
            </button>
            <span
              v-else
              class="shrink-0
                text-xs
                text-green-400/90"
              >{{ t('scriptEditor.composite.added') }}</span
            >
          </div>
        </div>
      </div>

      <button
        class="mt-2
          w-full
          rounded-lg
          border
          border-dashed
          border-white/15
          py-1.5
          text-xs
          text-white/45
          transition-all
          hover:border-brand
          hover:text-brand"
        @click="addRow({ text: '', actions: [] })"
      >
        ＋ {{ t('scriptEditor.composite.addOption') }}
      </button>
    </template>

    <!-- ============ chapter_end 的分支 ============ -->
    <template v-else-if="field.kind === 'branch_options'">
      <div
        v-for="(opt, i) in rows"
        :key="i"
        class="mb-2
          rounded-lg
          bg-white/6
          p-2.5
          last:mb-0"
      >
        <div
          v-if="!isAiJudged"
          class="mb-1.5
            flex
            items-center
            gap-2"
        >
          <span class="shrink-0
            text-xs
            text-white/40">{{
            t('scriptEditor.composite.ifPrefix')
          }}</span>
          <ConditionEditor
            class="flex-1
              min-w-0"
            :model-value="str(opt.condition)"
            :variables="store.variables"
            @update:model-value="(v: string) => patch(i, 'condition', v)"
          />
        </div>
        <div class="flex
          items-center
          gap-2
          pl-6">
          <span class="shrink-0
            text-xs
            text-white/40">{{
            t('scriptEditor.composite.jumpPrefix')
          }}</span>
          <select
            class="w-full
              min-w-0
              border
              border-white/[0.1]
              rounded-md
              bg-black/[0.25]
              px-2
              py-1.5
              text-xs
              text-white
              transition-all
              focus:outline-none
              focus:border-[var(--accent-color)]"
            :value="str(opt.next)"
            @change="(e) => patch(i, 'next', val(e))"
          >
            <option value="">{{ t('scriptEditor.composite.notSelected') }}</option>
            <option
              v-for="c in store.chapterOptions"
              :key="c.value"
              :value="c.value"
            >
              {{ c.label }}
            </option>
          </select>
          <label
            class="flex
              shrink-0
              items-center
              gap-1
              text-xs
              whitespace-nowrap
              text-white/60"
            :title="t('scriptEditor.composite.elseBranch')"
          >
            <input
              type="checkbox"
              :checked="opt.default === true"
              @change="(e) => patch(i, 'default', (e.target as HTMLInputElement).checked)"
            />
            {{ t('scriptEditor.composite.fallback') }}
          </label>
          <button
            class="shrink-0
              rounded-md
              px-1.5
              py-1
              text-xs
              text-white/[0.35]
              transition-all
              hover:text-[#fca5a5]
              hover:bg-[rgba(248,113,113,0.15)]"
            :title="t('scriptEditor.composite.deleteBranch')"
            @click="removeRow(i)"
          >
            ✕
          </button>
        </div>
        <div
          v-if="isAiJudged"
          class="mt-1.5
            flex
            items-center
            gap-2
            pl-6"
        >
          <span class="shrink-0
            text-xs
            text-white/40">{{
            t('scriptEditor.composite.aiAlias')
          }}</span>
          <input
            class="w-full
              min-w-0
              border
              border-white/[0.1]
              rounded-md
              bg-black/[0.25]
              px-2
              py-1.5
              text-xs
              text-white
              transition-all
              focus:outline-none
              focus:border-[var(--accent-color)]"
            :placeholder="t('scriptEditor.composite.branchName')"
            :value="str(opt.name)"
            @change="(e) => patch(i, 'name', val(e))"
          />
        </div>
      </div>

      <button
        class="mt-2
          w-full
          rounded-lg
          border
          border-dashed
          border-white/15
          py-1.5
          text-xs
          text-white/45
          transition-all
          hover:border-brand
          hover:text-brand"
        @click="addRow({ condition: '', next: '' })"
      >
        {{ t('scriptEditor.composite.addBranch') }}
      </button>
      <p class="mt-2
        text-xs
        text-white/40">
        {{ t('scriptEditor.composite.branchHelp') }}
      </p>
    </template>

    <!-- ============ set_variable 的赋值组 ============ -->
    <template v-else>
      <div
        v-for="(opt, i) in rows"
        :key="i"
        class="mb-2
          rounded-lg
          bg-white/6
          p-2.5
          last:mb-0"
      >
        <!-- 组条件：默认收起，点「＋ 条件」展开（与 choices 选项一致），
             避免与事件级「触发条件」混在一起 -->
        <div
          v-if="conditionOpen(i)"
          class="mb-1.5
            flex
            items-center
            gap-2"
        >
          <span class="shrink-0
            text-xs
            text-white/40">{{
            t('scriptEditor.composite.ifPrefix')
          }}</span>
          <ConditionEditor
            class="w-full
              min-w-0"
            :model-value="str(opt.condition)"
            :variables="store.variables"
            @update:model-value="(v: string) => patch(i, 'condition', v)"
          />
          <button
            class="shrink-0
              rounded-md
              px-1.5
              py-1
              text-xs
              text-white/[0.35]
              transition-all
              hover:text-[#fca5a5]
              hover:bg-[rgba(248,113,113,0.15)]"
            :title="t('scriptEditor.composite.removeCondition')"
            @click="closeCondition(i)"
          >
            ✕
          </button>
          <button
            class="shrink-0
              rounded-md
              px-1.5
              py-1
              text-xs
              text-white/[0.35]
              transition-all
              hover:text-[#fca5a5]
              hover:bg-[rgba(248,113,113,0.15)]"
            :title="t('scriptEditor.composite.deleteGroup')"
            @click="removeRow(i)"
          >
            ✕
          </button>
        </div>
        <div
          v-else
          class="mb-1.5
            flex
            items-center
            gap-2"
        >
          <button
            class="text-xs
              text-brand
              hover:underline"
            @click="openCondition(i)"
          >
            ＋ {{ t('scriptEditor.composite.addCondition') }}
          </button>
          <button
            class="ml-auto
              shrink-0
              rounded-md
              px-1.5
              py-1
              text-xs
              text-white/[0.35]
              transition-all
              hover:text-[#fca5a5]
              hover:bg-[rgba(248,113,113,0.15)]"
            :title="t('scriptEditor.composite.deleteGroup')"
            @click="removeRow(i)"
          >
            ✕
          </button>
        </div>
        <div
          v-for="(act, ai) in actions(opt)"
          :key="ai"
          class="mt-1
            flex
            items-center
            gap-2
            pl-6"
        >
          <VariableEditor
            v-if="act.type === 'set_var' && !legacySetVar(act)"
            class="flex-1
              min-w-0"
            :model-value="str(act.content)"
            :variables="store.variables"
            @update:model-value="(v: string) => patchAction(i, ai, 'content', v)"
          />
          <div
            v-else-if="legacySetVar(act)"
            class="flex-1
              min-w-0
              rounded-md
              border
              border-yellow-300/25
              bg-yellow-300/10
              px-2
              py-1.5"
          >
            <p class="text-xs
              text-yellow-200">
              {{ t('scriptEditor.composite.legacyNotice') }}
            </p>
            <button
              class="mt-1
                text-xs
                text-brand
                hover:underline"
              @click="convertLegacy(i, ai)"
            >
              {{ t('scriptEditor.composite.convert') }}
            </button>
          </div>
          <button
            class="shrink-0
              rounded-md
              px-1.5
              py-1
              text-xs
              text-white/[0.35]
              transition-all
              hover:text-[#fca5a5]
              hover:bg-[rgba(248,113,113,0.15)]"
            @click="removeAction(i, ai)"
          >
            ✕
          </button>
        </div>
        <button
          class="mt-1.5
            ml-6
            text-xs
            text-brand
            hover:underline"
          @click="addAction(i, 'set_var')"
        >
          ＋ {{ t('scriptEditor.composite.addAssignment') }}
        </button>
      </div>

      <button
        class="mt-2
          w-full
          rounded-lg
          border
          border-dashed
          border-white/15
          py-1.5
          text-xs
          text-white/45
          transition-all
          hover:border-brand
          hover:text-brand"
        @click="addRow({ actions: [{ type: 'set_var', content: '' }] })"
      >
        ＋ {{ t('scriptEditor.composite.addAssignmentGroup') }}
      </button>
      <p class="mt-2
        text-xs
        text-white/40">
        {{ t('scriptEditor.composite.assignmentHelp') }}
      </p>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive } from 'vue'
import { useI18n } from 'vue-i18n'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type { FieldSpec } from '@/api/services/script-editor'
import ConditionEditor from './ConditionEditor.vue'
import VariableEditor from './VariableEditor.vue'

type Row = Record<string, unknown>

const { t } = useI18n()
const props = defineProps<{
  field: FieldSpec
  value: unknown
  /** 分支编辑器的显示模式，由父组件按 chapter_end 的 end_type 传入 */
  branchMode?: 'branching' | 'ai_judged'
}>()

const emit = defineEmits<{ (e: 'update', value: unknown): void }>()

const store = useScriptEditorStore()

const rows = computed<Row[]>(() => (Array.isArray(props.value) ? (props.value as Row[]) : []))

const str = (v: unknown) => (typeof v === 'string' ? v : v === undefined ? '' : String(v))
const val = (e: Event) => (e.target as HTMLInputElement | HTMLSelectElement).value

const actions = (opt: Row): Row[] => (Array.isArray(opt.actions) ? (opt.actions as Row[]) : [])

/** 该选项是否已有一条「追加玩家台词」——有则隐藏对应按钮（每条选项最多一句玩家台词有意义） */
const hasAddLine = (i: number) => actions(rows.value[i]).some((a) => a.type === 'add_line')

/**
 * 分支编辑器模式。父组件在 end_type 为 ai_judged 时传 ai_judged，否则传
 * branching（linear 不显示分支，走不到这里）。null 兜底为 branching。
 */
const isAiJudged = computed(() => props.branchMode === 'ai_judged')

/**
 * 命中「旧原型形状」的 set_var 动作：只写了 name/value/op、没有 content 表达式。
 * 引擎只读 content，这类动作会被静默跳过（校验器也会报 action.legacy_shape）。
 * 编辑器里只读展示，提供「一键转为新格式」入口。
 */
const legacySetVar = (act: Row) =>
  act.type === 'set_var' &&
  !(typeof act.content === 'string' && act.content.trim()) &&
  Boolean(act.name || act.value || act.op)

/** 把旧版 name/value/op 合并成 content 表达式（name op value），并清掉旧字段 */
const convertLegacy = (i: number, ai: number) => {
  const next = clone()
  const list = Array.isArray(next[i]?.actions) ? (next[i].actions as Row[]) : []
  const act = list[ai]
  if (!act) return
  const expr = [act.name, act.op, act.value]
    .filter((v) => typeof v === 'string')
    .join(' ')
    .trim()
  if (!expr) return
  act.content = expr
  delete act.name
  delete act.value
  delete act.op
  commit(next)
}

/** choices 选项的条件行是否展开：已有条件 或 作者点过「＋ 条件」 */
const conditionOpenState = reactive<Record<number, boolean>>({})
const conditionOpen = (i: number) => {
  const has =
    typeof rows.value[i]?.condition === 'string' && str(rows.value[i]?.condition).trim() !== ''
  return has || conditionOpenState[i] === true
}
const openCondition = (i: number) => {
  conditionOpenState[i] = true
}
const closeCondition = (i: number) => {
  conditionOpenState[i] = false
  patch(i, 'condition', '')
}

const actionLabel = (type: string) =>
  type === 'set_var'
    ? t('scriptEditor.composite.addVariable')
    : t('scriptEditor.composite.playerLine')

const actionPlaceholder = (type: string) =>
  type === 'set_var' ? 'affection += 1' : t('scriptEditor.composite.playerLinePlaceholder')

/** 深拷贝后再改，避免直接 mutate 掉撤销栈里的旧帧 */
const clone = (): Row[] => JSON.parse(JSON.stringify(rows.value))

const commit = (next: Row[]) => emit('update', next)

const patch = (i: number, key: string, v: unknown) => {
  const next = clone()
  if (!next[i]) return
  if (v === '' || v === false) delete next[i][key]
  else next[i][key] = v
  commit(next)
}

const addRow = (row: Row) => commit([...clone(), row])

const removeRow = (i: number) => {
  const next = clone()
  next.splice(i, 1)
  commit(next)
}

const addAction = (i: number, type = 'add_line') => {
  const next = clone()
  if (!next[i]) return
  const list = Array.isArray(next[i].actions) ? (next[i].actions as Row[]) : []
  // 同一个选项里不允许重复添加「追加玩家台词」——每条选项最多一句玩家台词有意义
  if (type === 'add_line' && list.some((a) => a.type === 'add_line')) return
  // 追加玩家台词：默认复制上方选项文案，免去重复输入（一般保持一致即可）
  const content = type === 'add_line' ? str(rows.value[i]?.text) : ''
  list.push({ type, content })
  next[i].actions = list
  commit(next)
}

const patchAction = (i: number, ai: number, key: string, v: unknown) => {
  const next = clone()
  const list = Array.isArray(next[i]?.actions) ? (next[i].actions as Row[]) : []
  if (!list[ai]) return
  list[ai][key] = v
  commit(next)
}

const removeAction = (i: number, ai: number) => {
  const next = clone()
  const list = Array.isArray(next[i]?.actions) ? (next[i].actions as Row[]) : []
  list.splice(ai, 1)
  next[i].actions = list
  commit(next)
}
</script>

<style scoped>
/* select 里的 option 无法用 class 直接打进去，保留本文件唯一一条 scoped 规则 */
select option {
  background: #16202c;
  color: #fff;
}
</style>

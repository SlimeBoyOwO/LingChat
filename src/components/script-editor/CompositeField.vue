<template>
  <div class="rounded-xl border border-white/10 bg-black/15 p-3">
    <!-- ============ choices 的选项列表 ============ -->
    <template v-if="field.kind === 'choice_options'">
      <div
        v-for="(opt, i) in rows"
        :key="i"
        class="mb-2 rounded-lg bg-white/6 p-2.5 last:mb-0"
      >
        <div class="mb-1.5 flex items-center gap-2">
          <span class="text-xs text-white/40">{{ i + 1 }}</span>
          <input
            class="mini-input"
            placeholder="选项文案（留空＝匹配任意输入的兜底项，必须放最后）"
            :value="str(opt.text)"
            @change="(e) => patch(i, 'text', val(e))"
          />
          <button
            class="mini-btn"
            title="删除这个选项"
            @click="removeRow(i)"
          >
            ✕
          </button>
        </div>

        <div
          v-for="(act, ai) in actions(opt)"
          :key="ai"
          class="mt-1 flex items-center gap-2 pl-6"
        >
          <select
            class="mini-input w-32 shrink-0"
            :value="str(act.type)"
            @change="(e) => patchAction(i, ai, 'type', val(e))"
          >
            <option
              v-for="a in allowedActions"
              :key="a.typeKey"
              :value="a.typeKey"
            >
              {{ a.label }}
            </option>
          </select>
          <input
            class="mini-input"
            :placeholder="actionPlaceholder(str(act.type))"
            :value="str(act.content)"
            @change="(e) => patchAction(i, ai, 'content', val(e))"
          />
          <button
            class="mini-btn"
            @click="removeAction(i, ai)"
          >
            ✕
          </button>
        </div>

        <button
          class="mt-1.5 ml-6 text-xs text-brand hover:underline"
          @click="addAction(i)"
        >
          ＋ 添加动作
        </button>
      </div>

      <button
        class="mt-2 w-full rounded-lg border border-dashed border-white/15 py-1.5 text-xs
          text-white/45 transition-all hover:border-brand hover:text-brand"
        @click="addRow({ text: '', actions: [] })"
      >
        ＋ 添加选项
      </button>
    </template>

    <!-- ============ chapter_end 的分支 ============ -->
    <template v-else-if="field.kind === 'branch_options'">
      <div
        v-for="(opt, i) in rows"
        :key="i"
        class="mb-2 rounded-lg bg-white/6 p-2.5 last:mb-0"
      >
        <div class="mb-1.5 flex items-center gap-2">
          <span class="shrink-0 text-xs text-white/40">若</span>
          <input
            class="mini-input"
            placeholder="条件，如 route == shop（留空＝无条件命中）"
            :value="str(opt.condition)"
            @change="(e) => patch(i, 'condition', val(e))"
          />
          <button
            class="mini-btn"
            @click="removeRow(i)"
          >
            ✕
          </button>
        </div>
        <div class="flex items-center gap-2 pl-6">
          <span class="shrink-0 text-xs text-white/40">跳到</span>
          <select
            class="mini-input"
            :value="str(opt.next)"
            @change="(e) => patch(i, 'next', val(e))"
          >
            <option value="">（未选择）</option>
            <option
              v-for="c in store.chapterOptions"
              :key="c.value"
              :value="c.value"
            >
              {{ c.label }}
            </option>
          </select>
          <label class="flex shrink-0 items-center gap-1 text-xs whitespace-nowrap text-white/60">
            <input
              type="checkbox"
              :checked="opt.default === true"
              @change="(e) => patch(i, 'default', (e.target as HTMLInputElement).checked)"
            />
            兜底
          </label>
        </div>
        <div
          v-if="needsName"
          class="mt-1.5 flex items-center gap-2 pl-6"
        >
          <span class="shrink-0 text-xs text-white/40">AI 识别名</span>
          <input
            class="mini-input"
            placeholder="给 AI 看的分支名称"
            :value="str(opt.name)"
            @change="(e) => patch(i, 'name', val(e))"
          />
        </div>
      </div>

      <button
        class="mt-2 w-full rounded-lg border border-dashed border-white/15 py-1.5 text-xs
          text-white/45 transition-all hover:border-brand hover:text-brand"
        @click="addRow({ condition: '', next: '' })"
      >
        ＋ 添加分支
      </button>
      <p class="mt-2 text-xs text-white/40">
        顺序即优先级。所有条件都不满足且没有兜底分支时，剧本会直接结束。
      </p>
    </template>

    <!-- ============ set_variable 的赋值组 ============ -->
    <template v-else>
      <div
        v-for="(opt, i) in rows"
        :key="i"
        class="mb-2 rounded-lg bg-white/6 p-2.5 last:mb-0"
      >
        <div class="mb-1.5 flex items-center gap-2">
          <span class="shrink-0 text-xs text-white/40">若</span>
          <input
            class="mini-input"
            placeholder="条件（留空＝总是执行）"
            :value="str(opt.condition)"
            @change="(e) => patch(i, 'condition', val(e))"
          />
          <button
            class="mini-btn"
            @click="removeRow(i)"
          >
            ✕
          </button>
        </div>
        <div
          v-for="(act, ai) in actions(opt)"
          :key="ai"
          class="mt-1 flex items-center gap-2 pl-6"
        >
          <input
            class="mini-input font-mono"
            placeholder="affection += 1"
            :value="str(act.content)"
            @change="(e) => patchAction(i, ai, 'content', val(e))"
          />
          <button
            class="mini-btn"
            @click="removeAction(i, ai)"
          >
            ✕
          </button>
        </div>
        <button
          class="mt-1.5 ml-6 text-xs text-brand hover:underline"
          @click="addAction(i, 'set_var')"
        >
          ＋ 添加赋值
        </button>
      </div>

      <button
        class="mt-2 w-full rounded-lg border border-dashed border-white/15 py-1.5 text-xs
          text-white/45 transition-all hover:border-brand hover:text-brand"
        @click="addRow({ actions: [{ type: 'set_var', content: '' }] })"
      >
        ＋ 添加赋值组
      </button>
      <p class="mt-2 text-xs text-white/40">
        运算符只有 = / += / -=。与选项不同，这里所有满足条件的组都会执行。
      </p>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type { FieldSpec } from '@/api/services/script-editor'

type Row = Record<string, unknown>

const props = defineProps<{
  field: FieldSpec
  value: unknown
  /** ai_judged 时分支需要 name，由父组件按 end_type 传入 */
  needsName?: boolean
}>()

const emit = defineEmits<{ (e: 'update', value: unknown): void }>()

const store = useScriptEditorStore()

const rows = computed<Row[]>(() =>
  Array.isArray(props.value) ? (props.value as Row[]) : [],
)

const str = (v: unknown) => (typeof v === 'string' ? v : v === undefined ? '' : String(v))
const val = (e: Event) => (e.target as HTMLInputElement | HTMLSelectElement).value

const actions = (opt: Row): Row[] => (Array.isArray(opt.actions) ? (opt.actions as Row[]) : [])

/** set_variable 只支持 set_var —— 引擎里 add_line 会被静默丢弃 */
const allowedActions = computed(() => {
  const owner = props.field.kind === 'var_options' ? 'set_variable' : 'choices'
  return (store.schema?.actionTypes ?? []).filter((a) => a.allowedIn.includes(owner))
})

const actionPlaceholder = (type: string) =>
  type === 'set_var' ? 'affection += 1' : '写入对话历史的一句玩家台词'

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
  list.push({ type, content: '' })
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
.mini-input {
  width: 100%;
  min-width: 0;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 0.375rem;
  background: rgba(0, 0, 0, 0.25);
  padding: 0.375rem 0.5rem;
  font-size: 0.75rem;
  color: #fff;
  transition: all 0.2s;
}
.mini-input:focus {
  outline: none;
  border-color: var(--accent-color);
}
.mini-input option {
  background: #16202c;
  color: #fff;
}
.mini-btn {
  flex: 0 0 auto;
  border-radius: 0.375rem;
  padding: 0.25rem 0.375rem;
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.35);
  transition: all 0.2s;
}
.mini-btn:hover {
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.15);
}
</style>

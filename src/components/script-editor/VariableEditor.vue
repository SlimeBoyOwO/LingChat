<template>
  <div>
    <!-- 顶部说明行：传空串隐藏（说明已由父级承担时）；不传用默认通俗文案 -->
    <p
      v-if="displayHint"
      class="mb-1.5 text-xs text-white/35"
    >{{ displayHint }}</p>

    <!-- 无法解析的旧写法（如只写了 name/value/op 的旧形状）：只读展示 + 提供「清空重填」 -->
    <div v-if="parseError">
      <div class="flex items-center gap-2">
        <input
          class="flex-1 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white/50 opacity-70"
          :value="modelValue"
          readonly
        />
        <button
          class="shrink-0 rounded-md border border-white/[0.1] px-2 py-1.5 text-xs text-white/[0.7] transition-all hover:text-white hover:bg-white/[0.12]"
          @click="clear"
        >清空重填</button>
      </div>
      <p class="mt-1 text-xs text-yellow-200">
        这段赋值不是支持的写法（校验器会提示原因）。点「清空重填」后用下面的表单重新填写。
      </p>
    </div>

    <div v-else class="flex flex-wrap items-center gap-2">
      <input
        class="w-24 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        :list="uid"
        placeholder="变量名"
        :value="draft.var"
        @change="onVar"
      />
      <select
        class="shrink-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        :value="draft.op"
        @change="onOp"
      >
        <option value="=">设为</option>
        <option value="+=">加</option>
        <option value="-=">减</option>
      </select>
      <select
        class="shrink-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        :value="draft.kind"
        @change="onKind"
      >
        <option value="text">文本</option>
        <option value="number">数字</option>
        <option value="bool">布尔</option>
        <option value="random">随机数</option>
      </select>

      <input
        v-if="draft.kind === 'text'"
        class="w-32 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        placeholder="值"
        :value="draft.value"
        @change="onValue"
      />
      <input
        v-else-if="draft.kind === 'number'"
        class="w-24 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        type="number"
        placeholder="数值"
        :value="draft.value"
        @change="onValue"
      />
      <select
        v-else-if="draft.kind === 'bool'"
        class="shrink-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        :value="draft.value === 'true' ? 'true' : 'false'"
        @change="onBool"
      >
        <option value="true">真（true）</option>
        <option value="false">假（false）</option>
      </select>
      <template v-else-if="draft.kind === 'random'">
        <input
          class="w-16 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
          type="number"
          placeholder="最小"
          :value="String(draft.randomMin ?? '')"
          @change="onRandomMin"
        />
        <span class="shrink-0 text-xs text-white/40">到</span>
        <input
          class="w-16 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
          type="number"
          placeholder="最大"
          :value="String(draft.randomMax ?? '')"
          @change="onRandomMax"
        />
      </template>
      <p
        v-if="!draft.var.trim() || (draft.kind !== 'bool' && draft.kind !== 'random' && !draft.value.trim())"
        class="shrink-0 text-xs text-white/35"
      >填完整才算设置</p>
    </div>

    <datalist :id="uid">
      <option
        v-for="v in variables"
        :key="v"
        :value="v"
      ></option>
    </datalist>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { useId } from 'vue'
import {
  buildVarAction,
  parseVarAction,
  type VarOp,
  type VarParts,
  type VarValueKind,
} from '@/utils/scriptVar'

const props = defineProps<{
  /** 赋值表达式（引擎格式），如 flag = warm / count += 1 / 空串 */
  modelValue: string
  /** 已知变量名，供 datalist 补全 */
  variables: string[]
  /** 顶部说明行文案：传空串隐藏（说明由父级承担时）；不传用默认通俗文案 */
  hint?: string
}>()

const emit = defineEmits<{ (e: 'update:modelValue', value: string): void }>()

const uid = useId()

/** 默认文案覆盖各场景的共性语义；父级有更贴切的说明时可覆盖或隐藏 */
const displayHint = computed(() =>
  props.hint === undefined
    ? '这一条会把变量改成这个值，后面的条件判断会读到它'
    : props.hint,
)

/**
 * 内部草稿：编辑期间先落在本地，只有构成完整表达式（buildVarAction 非空）才
 * 写回 modelValue。否则「选了运算符但还没填值」这种中间态会被父级当成空值
 * 删键，整个表单被重置——这是之前「选了操作后输入框消失、无法保存」的根因。
 */
const draft = reactive<VarParts>(
  parseVarAction(props.modelValue) ?? { var: '', op: '=', kind: 'text', value: '' },
)

/** 外部值变化（撤销/重做/清空重填/切换事件）时同步草稿，但正在编辑时不打断输入 */
watch(
  () => props.modelValue,
  (v) => {
    const parsed = parseVarAction(v)
    if (parsed) {
      draft.var = parsed.var
      draft.op = parsed.op
      draft.kind = parsed.kind
      draft.value = parsed.value
      draft.randomMin = parsed.randomMin
      draft.randomMax = parsed.randomMax
    } else if (!v || !v.trim()) {
      // 外部清空（如「清空重填」）→ 重置表单
      draft.var = ''
      draft.op = '='
      draft.kind = 'text'
      draft.value = ''
      draft.randomMin = undefined
      draft.randomMax = undefined
    }
  },
)

const parseError = computed(() => {
  const s = (props.modelValue ?? '').trim()
  return s !== '' && parseVarAction(props.modelValue) === null
})

/** 只有表达式完整才写回；不完整时保留草稿，等作者继续填 */
const commit = () => {
  const s = buildVarAction(draft)
  if (s) emit('update:modelValue', s)
}

/** 清空重填：旧写法解析不出结构化表单，提供显式入口删掉它，再让作者重新填 */
const clear = () => emit('update:modelValue', '')

const onVar = (e: Event) => {
  draft.var = (e.target as HTMLInputElement).value
  commit()
}
const onValue = (e: Event) => {
  draft.value = (e.target as HTMLInputElement).value
  commit()
}
const onBool = (e: Event) => {
  draft.value = (e.target as HTMLSelectElement).value
  commit()
}
const onOp = (e: Event) => {
  draft.op = (e.target as HTMLSelectElement).value as VarOp
  commit()
}
const onKind = (e: Event) => {
  draft.kind = (e.target as HTMLSelectElement).value as VarValueKind
  commit()
}

const onRandomMin = (e: Event) => {
  const n = Number((e.target as HTMLInputElement).value)
  draft.randomMin = Number.isFinite(n) ? n : undefined
  commit()
}
const onRandomMax = (e: Event) => {
  const n = Number((e.target as HTMLInputElement).value)
  draft.randomMax = Number.isFinite(n) ? n : undefined
  commit()
}
</script>

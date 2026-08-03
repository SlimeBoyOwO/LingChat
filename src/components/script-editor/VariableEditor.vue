<template>
  <div>
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
        :value="part.var"
        @change="onVar"
      />
      <select
        class="shrink-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        :value="part.op"
        @change="onOp"
      >
        <option value="=">设为</option>
        <option value="+=">加</option>
        <option value="-=">减</option>
      </select>
      <select
        class="shrink-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        :value="part.kind"
        @change="onKind"
      >
        <option value="text">文本</option>
        <option value="number">数字</option>
        <option value="bool">布尔</option>
        <option value="random">随机数</option>
      </select>

      <input
        v-if="part.kind === 'text'"
        class="w-32 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        placeholder="值"
        :value="part.value"
        @change="onValue"
      />
      <input
        v-else-if="part.kind === 'number'"
        class="w-24 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        type="number"
        placeholder="数值"
        :value="part.value"
        @change="onValue"
      />
      <select
        v-else-if="part.kind === 'bool'"
        class="shrink-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        :value="part.value === 'true' ? 'true' : 'false'"
        @change="onBool"
      >
        <option value="true">真（true）</option>
        <option value="false">假（false）</option>
      </select>
      <template v-else-if="part.kind === 'random'">
        <input
          class="w-16 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
          type="number"
          placeholder="最小"
          :value="String(part.randomMin ?? '')"
          @change="onRandomMin"
        />
        <span class="shrink-0 text-xs text-white/40">到</span>
        <input
          class="w-16 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
          type="number"
          placeholder="最大"
          :value="String(part.randomMax ?? '')"
          @change="onRandomMax"
        />
      </template>
      <p
        v-if="!part.var.trim() || (part.kind !== 'bool' && part.kind !== 'random' && !part.value.trim())"
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
import { computed } from 'vue'
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
}>()

const emit = defineEmits<{ (e: 'update:modelValue', value: string): void }>()

const uid = useId()

const parseError = computed(() => {
  const s = (props.modelValue ?? '').trim()
  return s !== '' && parseVarAction(props.modelValue) === null
})

const part = computed<VarParts>(
  () => parseVarAction(props.modelValue) ?? { var: '', op: '=', kind: 'text', value: '' },
)

const commit = (next: VarParts) => emit('update:modelValue', buildVarAction(next))

/** 清空重填：旧写法解析不出结构化表单，提供显式入口删掉它，再让作者重新填 */
const clear = () => emit('update:modelValue', '')

const onVar = (e: Event) => commit({ ...part.value, var: (e.target as HTMLInputElement).value })
const onValue = (e: Event) => commit({ ...part.value, value: (e.target as HTMLInputElement).value })
const onBool = (e: Event) => commit({ ...part.value, value: (e.target as HTMLSelectElement).value })
const onOp = (e: Event) => commit({ ...part.value, op: (e.target as HTMLSelectElement).value as VarOp })
const onKind = (e: Event) =>
  commit({ ...part.value, kind: (e.target as HTMLSelectElement).value as VarValueKind })

const onRandomMin = (e: Event) => {
  const n = Number((e.target as HTMLInputElement).value)
  commit({ ...part.value, randomMin: Number.isFinite(n) ? n : undefined })
}
const onRandomMax = (e: Event) => {
  const n = Number((e.target as HTMLInputElement).value)
  commit({ ...part.value, randomMax: Number.isFinite(n) ? n : undefined })
}
</script>

<template>
  <div>
    <!-- 无法解析的旧写法：只读展示 + 交给校验器解释，提供「清空重填」入口 -->
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
        这段条件不是支持的写法（校验器会提示原因）。点「清空重填」后用下面的表单重新选择。
      </p>
    </div>

    <div v-else class="flex flex-wrap items-center gap-2">
      <input
        class="w-32 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        :list="uid"
        placeholder="变量名"
        :value="part.var"
        @change="onVar"
      />
      <select
        class="shrink-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        :value="part.rel"
        @change="onRel"
      >
        <option value="truthy">为真（判断存没存过）</option>
        <option value="eq">等于</option>
        <option value="neq">不等于</option>
      </select>
      <input
        v-if="part.rel !== 'truthy'"
        class="w-32 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
        placeholder="值"
        :value="part.value"
        @change="onValue"
      />
      <p
        v-if="part.rel !== 'truthy' && !part.value.trim()"
        class="shrink-0 text-xs text-white/35"
      >未填值＝未设置条件</p>
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
import { buildCondition, parseCondition, type ConditionParts, type ConditionRel } from '@/utils/scriptVar'

const props = defineProps<{
  /** 条件字符串（引擎格式），如 route == shop / flag / 空串 */
  modelValue: string
  /** 已知变量名，供 datalist 补全 */
  variables: string[]
}>()

const emit = defineEmits<{ (e: 'update:modelValue', value: string): void }>()

const uid = useId()

/** 非空但解析不出结构化 → 只读展示。空串（未设置）走正常空表单。 */
const parseError = computed(() => {
  const s = (props.modelValue ?? '').trim()
  return s !== '' && parseCondition(props.modelValue) === null
})

const part = computed<ConditionParts>(
  () => parseCondition(props.modelValue) ?? { var: '', rel: 'truthy', value: '' },
)

const commit = (next: ConditionParts) => emit('update:modelValue', buildCondition(next))

/** 清空重填：旧写法解析不出结构化表单，提供显式入口删掉它，再让作者重新选 */
const clear = () => emit('update:modelValue', '')

const onVar = (e: Event) => {
  const v = (e.target as HTMLInputElement).value
  commit({ ...part.value, var: v })
}

const onValue = (e: Event) => {
  const v = (e.target as HTMLInputElement).value
  commit({ ...part.value, value: v })
}

const onRel = (e: Event) => {
  commit({ ...part.value, rel: (e.target as HTMLSelectElement).value as ConditionRel })
}
</script>

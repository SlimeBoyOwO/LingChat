<template>
  <div>
    <!-- 顶部说明行：传空串隐藏（说明已由父级承担时）；不传用默认通俗文案 -->
    <p
      v-if="displayHint"
      class="mb-1.5
        text-xs
        text-white/35"
    >
      {{ displayHint }}
    </p>

    <!-- 无法解析的旧写法：只读展示 + 交给校验器解释，提供「清空重填」入口 -->
    <div v-if="parseError">
      <div class="flex
        items-center
        gap-2">
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
            text-white/50
            opacity-70"
          :value="modelValue"
          readonly
        />
        <button
          class="shrink-0
            rounded-md
            border
            border-white/[0.1]
            px-2
            py-1.5
            text-xs
            text-white/[0.7]
            transition-all
            hover:text-white
            hover:bg-white/[0.12]"
          @click="clear"
        >
          {{ t('scriptEditor.condition.clear') }}
        </button>
      </div>
      <p class="mt-1
        text-xs
        text-yellow-200">
        {{ t('scriptEditor.condition.invalidNotice') }}
      </p>
    </div>

    <div
      v-else
      class="flex
        flex-wrap
        items-center
        gap-2"
    >
      <input
        class="w-32
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
        :list="uid"
        :placeholder="t('scriptEditor.condition.varName')"
        :value="draft.var"
        @change="onVar"
      />
      <select
        class="shrink-0
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
        :value="draft.rel"
        @change="onRel"
      >
        <option value="truthy">{{ t('scriptEditor.condition.operatorSet') }}</option>
        <option value="eq">{{ t('scriptEditor.condition.operatorEq') }}</option>
        <option value="neq">{{ t('scriptEditor.condition.operatorNeq') }}</option>
      </select>
      <input
        v-if="draft.rel !== 'truthy'"
        class="w-32
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
        :placeholder="t('scriptEditor.condition.varValue')"
        :value="draft.value"
        @change="onValue"
      />
      <p
        v-if="draft.rel !== 'truthy' && !draft.value.trim()"
        class="shrink-0
          text-xs
          text-white/35"
      >
        {{ t('scriptEditor.condition.valueHint') }}
      </p>
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
import { useI18n } from 'vue-i18n'
import { useId } from 'vue'
import {
  buildCondition,
  parseCondition,
  type ConditionParts,
  type ConditionRel,
} from '@/utils/scriptVar'

const { t } = useI18n()
const props = defineProps<{
  /** 条件字符串（引擎格式），如 route == shop / flag / 空串 */
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
  props.hint === undefined ? t('scriptEditor.condition.help') : props.hint,
)

/**
 * 内部草稿：编辑期间先落在本地，只有构成完整表达式（buildCondition 非空）才
 * 写回 modelValue。否则「选了等于但还没填值」这种中间态会被父级当成空值
 * 删键，整个表单被重置——这是之前「选了关系后输入框消失、无法保存」的根因。
 */
const draft = reactive<ConditionParts>(
  parseCondition(props.modelValue) ?? { var: '', rel: 'truthy', value: '' },
)

/** 外部值变化（撤销/重做/清空重填/切换事件）时同步草稿，但正在编辑时不打断输入 */
watch(
  () => props.modelValue,
  (v) => {
    const parsed = parseCondition(v)
    if (parsed) {
      draft.var = parsed.var
      draft.rel = parsed.rel
      draft.value = parsed.value
    } else if (!v || !v.trim()) {
      // 外部清空（如「清空重填」）→ 重置表单
      draft.var = ''
      draft.rel = 'truthy'
      draft.value = ''
    }
  },
)

/** 非空但解析不出结构化 → 只读展示。空串（未设置）走正常空表单。 */
const parseError = computed(() => {
  const s = (props.modelValue ?? '').trim()
  return s !== '' && parseCondition(props.modelValue) === null
})

/** 只有表达式完整才写回；不完整时保留草稿，等作者继续填 */
const commit = () => {
  const s = buildCondition(draft)
  if (s) emit('update:modelValue', s)
}

/** 清空重填：旧写法解析不出结构化表单，提供显式入口删掉它，再让作者重新选 */
const clear = () => emit('update:modelValue', '')

const onVar = (e: Event) => {
  draft.var = (e.target as HTMLInputElement).value
  commit()
}

const onValue = (e: Event) => {
  draft.value = (e.target as HTMLInputElement).value
  commit()
}

const onRel = (e: Event) => {
  draft.rel = (e.target as HTMLSelectElement).value as ConditionRel
  commit()
}
</script>

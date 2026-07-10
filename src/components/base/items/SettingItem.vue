<template>
  <!-- Case: 布尔值 (Checkbox) -->
  <template v-if="setting.type === 'bool'">
    <label
      class="inline-flex items-center cursor-pointer font-medium text-brand"
      :for="setting.key"
      >{{ setting.description || '' }}</label
    >
    <div class="flex align-baseline py-2.5 px-1">
      <Toggle :checked="setting.value.toLowerCase() === 'true'" @change="handleCheckboxChange">
      </Toggle>
      <p v-if="showInternalKey" class="text-sm text-gray-300">
        {{ setting.key }}
      </p>
    </div>
  </template>

  <!-- Case: 文本域 (Textarea) -->
  <template v-else-if="setting.type === 'textarea'">
    <label
      class="inline-flex items-center cursor-pointer font-medium text-brand"
      :for="setting.key"
      >{{ setting.description || '支持多行输入' }}</label
    >
    <p v-if="showInternalKey" class="text-sm mt-1 mb-2 text-gray-300">
      {{ setting.key }}
    </p>
    <textarea
      :id="setting.key"
      v-model="localValue"
      class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
      rows="8"
    ></textarea>
  </template>

  <!-- Case: 下拉选择 (Select) -->
  <template v-else-if="setting.type === 'select'">
    <label
      class="inline-flex items-center cursor-pointer font-medium text-brand"
      :for="setting.key"
      >{{ setting.description || '' }}</label
    >
    <p v-if="showInternalKey" class="text-sm mt-1 mb-2 text-gray-300">
      {{ setting.key }}
    </p>
    <select
      :id="setting.key"
      v-model="localValue"
      class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
    >
      <option
        v-for="opt in setting.options"
        :key="opt"
        :value="opt"
        class="bg-slate-900 text-white"
      >
        {{ selectOptionLabel(opt) }}
      </option>
    </select>
  </template>

  <!-- Case: 默认文本 (Text Input) -->
  <template v-else>
    <label
      class="inline-flex items-center cursor-pointer font-medium text-brand"
      :for="setting.key"
      >{{ setting.description || '' }}</label
    >
    <p v-if="showInternalKey" class="text-sm mt-1 mb-2 text-gray-300">
      {{ setting.key }}
    </p>
    <!-- 如果是 path 类型，添加文件选择按钮 -->
    <div v-if="setting.type === 'path'" class="flex gap-2">
      <input
        type="text"
        :id="setting.key"
        v-model="setting.value"
        class="flex-1 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
      />
      <button
        @click="selectFile(setting)"
        type="button"
        class="px-4 py-2.5 bg-brand text-white rounded-lg hover:bg-[#0056b3] transition-colors duration-200 whitespace-nowrap"
      >
        浏览
      </button>
    </div>
    <div v-else>
      <input
        :type="isWindowDimension ? 'number' : 'text'"
        :id="setting.key"
        v-model="localValue"
        :min="dimensionMin"
        :step="isWindowDimension ? 1 : undefined"
        :inputmode="isWindowDimension ? 'numeric' : undefined"
        :readonly="readonly"
        :aria-describedby="helperText ? `${setting.key}-help` : undefined"
        class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
        :class="{
          'cursor-not-allowed opacity-70 bg-slate-900/60': readonly,
        }"
      />
      <p
        v-if="helperText"
        :id="`${setting.key}-help`"
        class="mt-2 text-sm leading-5 text-slate-200"
      >
        {{ helperText }}
      </p>
    </div>
  </template>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import Toggle from '../widget/Toggle.vue'

interface Setting {
  key: string
  value: string
  type: 'bool' | 'textarea' | 'text' | 'path' | 'select'
  description?: string
  options?: string[]
}

const presetLabelMap: Record<string, string> = {
  fit: '适应当前显示器（推荐，保存时计算）',
  default: '默认（内容区 1500×800）',
  '1920x1080': '宽大（内容区 1920×1080）',
  '2560x1440': '超大（QHD 内容区 2560×1440）',
  '1280x720': '紧凑（内容区 1280×720）',
  custom: '自定义',
}

const selectOptionLabel = (opt: string): string => presetLabelMap[opt] || opt

interface Props {
  setting: Setting
  readonly?: boolean
  helperText?: string
}

const props = withDefaults(defineProps<Props>(), {
  readonly: false,
  helperText: '',
})

const windowSettingKeys = new Set([
  'ui.window_resolution_preset',
  'ui.window_width',
  'ui.window_height',
])
const showInternalKey = computed(() => !windowSettingKeys.has(props.setting.key))
const isWindowDimension = computed(
  () => props.setting.key === 'ui.window_width' || props.setting.key === 'ui.window_height',
)
const dimensionMin = computed(() => {
  if (props.setting.key === 'ui.window_width') return 1024
  if (props.setting.key === 'ui.window_height') return 640
  return undefined
})

const emit = defineEmits<{
  'update:value': [value: string]
}>()

const localValue = ref(props.setting.value)

// 监听本地值的变化，并触发更新事件
watch(localValue, (newValue) => {
  emit('update:value', newValue)
})

// 监听props.setting.value的变化，同步到本地值
watch(
  () => props.setting.value,
  (newValue) => {
    localValue.value = newValue
  },
)

// 处理复选框的变化
const handleCheckboxChange = (checked: boolean) => {
  const newValue = checked ? 'true' : 'false'
  localValue.value = newValue
  emit('update:value', newValue)
}

const selectFile = async (setting: { key: string; value: string }) => {
  try {
    const path = await invoke<string | null>('select_file')
    if (path) {
      setting.value = path
    }
  } catch (error: any) {
    console.error('文件选择失败:', error)
  }
}
</script>

<template>
  <!-- Case: 布尔值 (Checkbox) -->
  <template v-if="setting.type === 'bool'">
    <label
      class="inline-flex items-center cursor-pointer font-medium text-brand"
      :for="setting.key"
      >{{ setting.key }}</label
    >
    <div class="flex align-baseline py-2.5 px-1">
      <Toggle :checked="setting.value.toLowerCase() === 'true'" @change="handleCheckboxChange">
      </Toggle>
      <p class="text-sm text-gray-300">
        {{ setting.description || '' }}
      </p>
    </div>
  </template>

  <!-- Case: 文本域 (Textarea) -->
  <template v-else-if="setting.type === 'textarea'">
    <label
      class="inline-flex items-center cursor-pointer font-medium text-brand"
      :for="setting.key"
      >{{ setting.key }}</label
    >
    <p class="text-sm mt-1 mb-2 text-gray-300">
      {{ setting.description || '支持多行输入' }}
    </p>
    <textarea
      :id="setting.key"
      v-model="localValue"
      class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
      rows="8"
    ></textarea>
  </template>

  <!-- Case: 默认文本 (Text Input) -->
  <template v-else>
    <label
      class="inline-flex items-center cursor-pointer font-medium text-brand"
      :for="setting.key"
      >{{ setting.key }}</label
    >
    <p class="text-sm mt-1 mb-2 text-gray-300">
      {{ setting.description || '' }}
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
    <input
      type="text"
      :id="setting.key"
      v-model="localValue"
      class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
    />
  </template>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import Toggle from '../widget/Toggle.vue'

interface Setting {
  key: string
  value: string
  type: 'bool' | 'textarea' | 'text' | 'path'
  description?: string
}

interface Props {
  setting: Setting
}

const props = defineProps<Props>()

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
    const response = await fetch('/api/settings/select-file')

    const result = await response.json()

    if (result.error) {
      // 显示后端返回的错误信息
      console.error('后端错误:', result.error)
    } else if (result.path) {
      setting.value = result.path
    } else {
      // 用户取消选择
    }
  } catch (error: any) {
    console.error('文件选择失败:', error)
  }
}
</script>

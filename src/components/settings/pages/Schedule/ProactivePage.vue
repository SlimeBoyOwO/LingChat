<template>
  <!-- 视图：日程主题列表 -->
  <div
    v-if="uiStore.scheduleView === 'proactive_settings'"
    class="grid grid-cols-1 sm:grid-cols-1 lg:grid-cols-1 p-1"
  >
    <div v-for="setting in settings" :key="setting.key" class="mb-6">
      <!-- 使用 SettingItem 组件渲染不同类型的输入控件 -->
      <SettingItem :setting="setting" @update:value="(value) => (setting.value = value)" />
    </div>

    <div class="flex gap-2 align-text-bottom w-auto h-auto items-center">
      <div
        class="w-18 px-5 py-2.5 bg-brand text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#0056b3]"
      >
        <button @click="saveSettings">保存</button>
      </div>
      <p :style="{ color: saveStatus.color }">
        {{ saveStatus.message }}
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import { getEnvConfigByKey, saveEnvConfigSettings } from '@/api/services/config'
import { reloadProactiveSystem } from '@/api/services/schedule'
import type { ConfigItem } from '@/api/services/config'
import SettingItem from '@/components/base/items/SettingItem.vue'
const uiStore = useUIStore()
const settings = ref<Record<string, ConfigItem>>({})
const saveStatus = reactive({
  message: '',
  color: 'var(--success-color)',
})

const saveSettings = async () => {
  // 将 settings 转换为 Record<string, string> 格式
  const formData: Record<string, string> = {}
  Object.entries(settings.value).forEach(([key, config]) => {
    formData[key] = config.value
  })

  saveStatus.message = ''

  try {
    saveStatus.message = (await saveEnvConfigSettings(formData)).message
    saveStatus.color = 'var(--success-color)'
    reloadProactiveSystem()

    await loadConfig()
  } catch (error: any) {
    saveStatus.message = `错误: ${error.message}`
    saveStatus.color = 'red'
  } finally {
    setTimeout(() => {
      saveStatus.message = ''
    }, 5000)
  }
}

const loadConfig = async () => {
  const configKeys = [
    'ENABLE_PROACTIVE_SYSTEM',
    'MAX_PROACTIVE_TIMES',
    'VD_API_KEY',
    'VD_BASE_URL',
    'VD_MODEL',
    'ENABLE_VISUAL_PRECEPTION',
    'SCREEN_WEIGHT',
    'ENABLE_TOPIC_CREATER',
    'TOPIC_WEIGHT',
    'ENABLE_TODO_PRECEPTION',
    'TODO_WEIGHT',
    'ENABLE_SCHEDULE_REMINDER',
    'ENABLE_IMPORTANT_DAY_REMINDER',
  ]

  for (const key of configKeys) {
    settings.value[key] = await getEnvConfigByKey(key)
  }
}

onMounted(async () => {
  loadConfig()
})
</script>

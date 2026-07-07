<template>
  <!-- 视图：日程主题列表 -->
  <div
    v-if="uiStore.scheduleView === 'proactive_settings'"
    class="grid grid-cols-1 sm:grid-cols-1 lg:grid-cols-1 p-1"
  >
    <div v-for="setting in visibleSettings" :key="setting.key" class="mb-6">
      <!-- 使用 SettingItem 组件渲染不同类型的输入控件 -->
      <SettingItem :setting="setting" @update:value="(value) => (setting.value = value)" />
    </div>

    <div class="flex gap-3 align-text-bottom w-auto h-auto items-center">
      <button
        class="px-5 py-2.5 bg-brand text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#0056b3]"
        @click="saveSettings"
      >
        保存
      </button>
      <p :style="{ color: saveStatus.color }" class="text-sm">
        {{ saveStatus.message }}
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive, computed } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import { getEnvConfigByKey, saveEnvConfigSettings } from '@/api/services/config'
import { reloadProactiveSystem } from '@/api/services/schedule'
import type { ConfigItem } from '@/api/services/config'
import SettingItem from '@/components/base/items/SettingItem.vue'
const uiStore = useUIStore()
const settings = ref<Record<string, ConfigItem>>({})
const hiddenKeysWhenFollowing = ['VD_API_KEY', 'VD_BASE_URL', 'VD_MODEL']
const visibleSettings = computed(() => {
  const followChatModel = settings.value['VD_FOLLOW_CHAT_MODEL']?.value?.toLowerCase() === 'true'
  return Object.values(settings.value).filter((s) => {
    if (!followChatModel) return true
    return !hiddenKeysWhenFollowing.includes(s.key)
  })
})
const SUCCESS_COLOR = '#4ade80'

const saveStatus = reactive({
  message: '',
  color: SUCCESS_COLOR,
})

const saveSettings = async () => {
  // 将 settings 转换为 Record<string, string> 格式
  const formData: Record<string, string> = {}
  Object.entries(settings.value).forEach(([key, config]) => {
    formData[key] = config.value
  })

  saveStatus.message = ''

  try {
    await saveEnvConfigSettings(formData)
    saveStatus.message = '保存成功'
    saveStatus.color = SUCCESS_COLOR

    // 尝试重载主动系统；即使系统尚未运行也不影响保存结果
    try {
      await reloadProactiveSystem()
    } catch (e: any) {
      console.warn('重载主动系统失败（可忽略）:', e.message)
    }

    await loadConfig()
  } catch (error: any) {
    saveStatus.message = `错误: ${error.message}`
    saveStatus.color = '#ef4444'
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
    'PROACTIVE_INTERVAL_SECS',
    'INTEREST_TRIGGER_THRESHOLD',
    'INTEREST_DECAY_STEP',
    'VD_FOLLOW_CHAT_MODEL',
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

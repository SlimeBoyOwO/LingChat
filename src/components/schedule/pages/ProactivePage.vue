<template>
  <!-- 视图：日程主题列表 -->
  <div
    v-if="uiStore.scheduleView === 'proactive_settings'"
    class="grid grid-cols-1 sm:grid-cols-1 lg:grid-cols-1 p-1"
  >
    <div v-if="settings['ENABLE_PROACTIVE_SYSTEM']" class="mb-6">
      <SettingItem
        :setting="settings['ENABLE_PROACTIVE_SYSTEM']"
        @update:value="(value) => (settings['ENABLE_PROACTIVE_SYSTEM'].value = value)"
      />
    </div>

    <div class="flex flex-col gap-3">
      <div class="flex gap-3 align-text-bottom w-auto h-auto items-center">
        <button
          class="px-5 py-2.5 bg-brand text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#0056b3]"
          @click="saveSettings"
        >
          保存
        </button>
        <button
          class="px-5 py-2.5 bg-[#6366f1] text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#4f46e5] disabled:opacity-50 disabled:cursor-not-allowed"
          :disabled="testStatus.loading"
          @click="handleTestProactive"
        >
          {{ testStatus.loading ? '测试中...' : '测试主动消息' }}
        </button>
        <button
          class="px-5 py-2.5 bg-slate-600 text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-slate-500"
          @click="openAdvancedSettings"
        >
          更多设置
        </button>
        <p :style="{ color: saveStatus.color }" class="text-sm">
          {{ saveStatus.message }}
        </p>
        <p :style="{ color: testStatus.color }" class="text-sm">
          {{ testStatus.message }}
        </p>
      </div>
      <p class="text-xs text-white/60">
        更多主动对话参数（视觉模型、兴趣度阈值、话题权重等）请前往「更多设置」调整。
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import { getEnvConfigByKey, saveEnvConfigSettings } from '@/api/services/config'
import { reloadProactiveSystem, testProactiveMessage } from '@/api/services/schedule'
import type { ConfigItem } from '@/api/services/config'
import SettingItem from '@/components/base/items/SettingItem.vue'
const uiStore = useUIStore()
const settings = ref<Record<string, ConfigItem>>({})
const SUCCESS_COLOR = '#4ade80'

const saveStatus = reactive({
  message: '',
  color: SUCCESS_COLOR,
})

const testStatus = reactive({
  message: '',
  color: SUCCESS_COLOR,
  loading: false,
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

const openAdvancedSettings = () => {
  // 打开高级设置并直接切换到"其他高级设置"页签，方便用户找到主动对话配置
  uiStore.setAdvanceInitialTab('other')
  uiStore.setSettingsTab('advance')
  uiStore.toggleSettings(true)
}

const loadConfig = async () => {
  const configKeys = ['ENABLE_PROACTIVE_SYSTEM']

  for (const key of configKeys) {
    settings.value[key] = await getEnvConfigByKey(key)
  }
}

const handleTestProactive = async () => {
  testStatus.loading = true
  testStatus.message = ''

  try {
    const result = await testProactiveMessage()
    testStatus.message = result
    testStatus.color = SUCCESS_COLOR
  } catch (error: any) {
    testStatus.message = `测试失败: ${error.message || error}`
    testStatus.color = '#ef4444'
  } finally {
    testStatus.loading = false
    setTimeout(() => {
      testStatus.message = ''
    }, 6000)
  }
}

onMounted(async () => {
  loadConfig()
})
</script>

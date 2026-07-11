<template>
  <!-- 视图：日程主题列表 -->
  <div
    v-if="uiStore.scheduleView === 'proactive_settings'"
    class="grid grid-cols-1 sm:grid-cols-1 lg:grid-cols-1 p-1"
  >
    <div v-if="settings['ENABLE_PROACTIVE_SYSTEM']" class="mb-5">
      <SettingItem
        :setting="settings['ENABLE_PROACTIVE_SYSTEM']"
        @update:value="(value) => (settings['ENABLE_PROACTIVE_SYSTEM'].value = value)"
      />
    </div>

    <div class="grid grid-cols-1 gap-x-8 gap-y-5 md:grid-cols-2 mb-5">
      <template v-for="key in performanceSettingKeys" :key="key">
        <div v-if="settings[key]" class="min-w-0">
          <SettingItem
            :setting="settings[key]"
            @update:value="(value) => (settings[key].value = value)"
          />
        </div>
      </template>
    </div>

    <p v-if="configLoading" class="mb-4 text-sm text-white/65">正在加载主动对话配置...</p>

    <div class="flex flex-col gap-3">
      <p class="max-w-[72ch] text-xs leading-5 text-amber-200/80">
        速度提示：跟随开启思考的聊天模型会更慢；追求响应速度时，可关闭“跟随当前对话模型”，并在「更多设置」中配置独立的轻量视觉模型。
      </p>

      <div class="flex flex-wrap gap-3 align-text-bottom w-auto h-auto items-center">
        <button
          class="px-5 py-2.5 bg-brand text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#0056b3] disabled:opacity-50 disabled:cursor-not-allowed"
          :disabled="saving || testStatus.loading || configLoading"
          @click="saveSettings"
        >
          {{ saving ? '保存并应用中...' : '保存并应用' }}
        </button>
        <button
          class="px-5 py-2.5 bg-[#6366f1] text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#4f46e5] disabled:opacity-50 disabled:cursor-not-allowed"
          :disabled="testStatus.loading || saving || configLoading"
          @click="handleTestProactive"
        >
          {{
            testStatus.loading
              ? `测试中 ${testStatus.elapsedSeconds.toFixed(1)}s`
              : '测试主动消息'
          }}
        </button>
        <button
          class="px-5 py-2.5 bg-slate-600 text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-slate-500 disabled:opacity-50 disabled:cursor-not-allowed"
          :disabled="saving || testStatus.loading"
          @click="openAdvancedSettings"
        >
          更多设置
        </button>
      </div>

      <p v-if="testStatus.loading" class="text-sm text-indigo-200">
        阶段：{{ testStatus.stage }}，已耗时 {{ testStatus.elapsedSeconds.toFixed(1) }} 秒
      </p>
      <p v-if="saveStatus.message" :style="{ color: saveStatus.color }" class="text-sm">
        {{ saveStatus.message }}
      </p>
      <p v-if="testStatus.message" :style="{ color: testStatus.color }" class="text-sm">
        {{ testStatus.message }}
      </p>
      <p class="text-xs text-white/60">
        更多主动对话参数（视觉模型、兴趣度阈值、话题权重等）请前往「更多设置」调整。
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, reactive } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import { getEnvConfigByKey, saveEnvConfigSettings } from '@/api/services/config'
import { reloadProactiveSystem, testProactiveMessage } from '@/api/services/schedule'
import type { ConfigItem } from '@/api/services/config'
import SettingItem from '@/components/base/items/SettingItem.vue'
const uiStore = useUIStore()
const settings = ref<Record<string, ConfigItem>>({})
const SUCCESS_COLOR = '#4ade80'
const ERROR_COLOR = '#ef4444'
const quickSettingKeys = [
  'ENABLE_PROACTIVE_SYSTEM',
  'PROACTIVE_INTERVAL_SECS',
  'VD_FOLLOW_CHAT_MODEL',
  'VISUAL_PERCEPTION_PRIORITY',
  'SCREEN_WEIGHT',
] as const
const performanceSettingKeys = quickSettingKeys.slice(1)
const saving = ref(false)
const configLoading = ref(false)
let saveStatusTimer: number | null = null
let testStatusTimer: number | null = null
let testProgressTimer: number | null = null

const saveStatus = reactive({
  message: '',
  color: SUCCESS_COLOR,
})

const testStatus = reactive({
  message: '',
  color: SUCCESS_COLOR,
  loading: false,
  stage: '',
  elapsedSeconds: 0,
})

const errorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error || '未知错误')

const clearSaveStatusLater = () => {
  if (saveStatusTimer !== null) window.clearTimeout(saveStatusTimer)
  saveStatusTimer = window.setTimeout(() => {
    saveStatus.message = ''
    saveStatusTimer = null
  }, 8000)
}

const persistAndReloadSettings = async () => {
  const formData: Record<string, string> = {}
  Object.entries(settings.value).forEach(([key, config]) => {
    formData[key] = config.value
  })

  await saveEnvConfigSettings(formData)

  try {
    await reloadProactiveSystem()
  } catch (error) {
    throw new Error(`配置已保存，但主动对话系统重载失败，当前运行实例尚未应用：${errorMessage(error)}`)
  }

  await loadConfig()
}

const saveSettings = async () => {
  if (saving.value || testStatus.loading) return

  saving.value = true
  saveStatus.message = '正在保存并重载主动对话系统...'
  saveStatus.color = '#a5b4fc'

  try {
    await persistAndReloadSettings()
    saveStatus.message = '配置已保存，主动对话系统已应用最新设置'
    saveStatus.color = SUCCESS_COLOR
  } catch (error) {
    saveStatus.message = `应用失败：${errorMessage(error)}`
    saveStatus.color = ERROR_COLOR
  } finally {
    saving.value = false
    clearSaveStatusLater()
  }
}

const openAdvancedSettings = () => {
  // 打开高级设置并直接切换到"其他高级设置"页签，方便用户找到主动对话配置
  uiStore.setAdvanceInitialTab('other')
  uiStore.setSettingsTab('advance')
  uiStore.toggleSettings(true)
}

const loadConfig = async () => {
  configLoading.value = true
  try {
    const entries = await Promise.all(
      quickSettingKeys.map(async (key) => [key, await getEnvConfigByKey(key)] as const),
    )
    settings.value = Object.fromEntries(entries)
  } finally {
    configLoading.value = false
  }
}

const handleTestProactive = async () => {
  if (testStatus.loading || saving.value) return

  testStatus.loading = true
  testStatus.message = ''
  testStatus.color = SUCCESS_COLOR
  testStatus.stage = '正在应用当前页面的设置'
  testStatus.elapsedSeconds = 0

  if (testStatusTimer !== null) {
    window.clearTimeout(testStatusTimer)
    testStatusTimer = null
  }

  const startedAt = performance.now()
  const updateElapsed = () => {
    testStatus.elapsedSeconds = (performance.now() - startedAt) / 1000
  }
  testProgressTimer = window.setInterval(updateElapsed, 100)

  try {
    await persistAndReloadSettings()
    testStatus.stage = '完整链路处理中（截图 → 视觉识别 → 回复生成）'
    const result = await testProactiveMessage()
    updateElapsed()
    testStatus.stage = '测试完成'
    testStatus.message = `${result}（总耗时 ${testStatus.elapsedSeconds.toFixed(1)} 秒）`
    testStatus.color = SUCCESS_COLOR
  } catch (error) {
    updateElapsed()
    testStatus.stage = '测试失败'
    testStatus.message = `测试失败（${testStatus.elapsedSeconds.toFixed(1)} 秒）：${errorMessage(error)}`
    testStatus.color = ERROR_COLOR
  } finally {
    if (testProgressTimer !== null) {
      window.clearInterval(testProgressTimer)
      testProgressTimer = null
    }
    testStatus.loading = false
    testStatusTimer = window.setTimeout(() => {
      testStatus.message = ''
      testStatus.stage = ''
      testStatusTimer = null
    }, 12000)
  }
}

onMounted(async () => {
  try {
    await loadConfig()
  } catch (error) {
    saveStatus.message = `加载主动对话配置失败：${errorMessage(error)}`
    saveStatus.color = ERROR_COLOR
  }
})

onUnmounted(() => {
  if (saveStatusTimer !== null) window.clearTimeout(saveStatusTimer)
  if (testStatusTimer !== null) window.clearTimeout(testStatusTimer)
  if (testProgressTimer !== null) window.clearInterval(testProgressTimer)
})
</script>

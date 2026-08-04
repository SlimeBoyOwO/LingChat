<template>
  <!-- 高级设置 → 工具配置：网页搜索等聊天工具的设置 -->
  <div class="h-full overflow-y-auto custom-scrollbar p-2">
    <h2 class="text-2xl text-brand font-semibold pb-4 mb-6 border-b border-brand">
      {{ $t('ui.toolCalls.webSearchTitle') }}
    </h2>

    <div class="mb-6">
      <div class="flex items-center gap-3 py-2.5 px-1">
        <Toggle
          :checked="form.web_search.enabled"
          @change="(value: boolean) => (form.web_search.enabled = value)"
        />
        <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.enableWebSearch') }}</p>
      </div>

      <div class="flex items-center gap-3 py-2.5 px-1">
        <Toggle
          :checked="form.web_search.use_builtin"
          @change="(value: boolean) => (form.web_search.use_builtin = value)"
        />
        <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.useBuiltin') }}</p>
      </div>

      <p v-if="form.web_search.use_builtin" class="text-sm text-gray-400 px-1 mb-2">
        {{ $t('ui.toolCalls.builtinHint') }}
      </p>

      <div class="flex items-center gap-3 py-2.5 px-1">
        <Toggle
          :checked="form.web_search.hide_search_results"
          @change="(value: boolean) => (form.web_search.hide_search_results = value)"
        />
        <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.hideSearchResults') }}</p>
      </div>

      <template v-if="!form.web_search.use_builtin">
        <label class="inline-flex items-center font-medium text-brand mt-2">
          {{ $t('ui.toolCalls.apiKey') }}
        </label>
        <p class="text-sm mt-1 mb-2 text-gray-300">Moonshot API Key</p>
        <input
          type="password"
          v-model="form.web_search.api_key"
          :placeholder="$t('ui.toolCalls.apiKeyPlaceholder')"
          class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
        />

        <label class="inline-flex items-center font-medium text-brand mt-4">
          {{ $t('ui.toolCalls.baseUrl') }}
        </label>
        <input
          type="text"
          v-model="form.web_search.base_url"
          placeholder="https://api.kimi.com/coding/v1/search"
          class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
        />

        <label class="inline-flex items-center font-medium text-brand mt-4">
          {{ $t('ui.toolCalls.maxResults') }}
        </label>
        <input
          type="number"
          v-model.number="form.web_search.max_results"
          min="1"
          max="20"
          step="1"
          class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
        />
      </template>

      <div class="flex items-center gap-3 py-2.5 px-1 mt-2">
        <Toggle
          :checked="form.web_search.proxy_enabled"
          @change="(value: boolean) => (form.web_search.proxy_enabled = value)"
        />
        <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.proxyEnable') }}</p>
      </div>
      <input
        v-if="form.web_search.proxy_enabled"
        type="text"
        v-model="form.web_search.proxy_addr"
        placeholder="http://127.0.0.1:10808"
        class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
      />

      <!-- 其他工具（分组开关） -->
      <h2 class="text-2xl text-brand font-semibold pb-4 mt-8 mb-4 border-b border-brand">
        {{ $t('ui.toolCalls.otherToolsTitle') }}
      </h2>
      <p class="text-sm text-gray-400 mb-2 px-1">{{ $t('ui.toolCalls.otherToolsHint') }}</p>
      <div
        v-for="group in TOOL_GROUP_KEYS"
        :key="group"
        class="flex items-center gap-3 py-2.5 px-1"
      >
        <Toggle
          :checked="form.groups[group] ?? false"
          @change="(value: boolean) => (form.groups[group] = value)"
        />
        <p class="text-sm text-gray-300">{{ $t(`ui.toolCalls.groups.${group}`) }}</p>
      </div>

      <div class="flex gap-2 items-center mt-4">
        <div
          class="w-18 px-5 py-2.5 bg-brand text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#0056b3]"
          @click="saveSettings"
        >
          {{ $t('ui.toolCalls.save') }}
        </div>
        <div
          class="px-5 py-2.5 bg-white/10 text-white border border-white/20 rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-white/20"
          @click="runTest"
        >
          {{ $t('ui.toolCalls.test') }}
        </div>
        <p class="text-sm" :style="{ color: status.color }">{{ status.message }}</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  getToolSettings,
  saveToolSettings,
  testWebSearch,
  TOOL_GROUP_KEYS,
  type ToolSettings,
} from '@/api/services/tool-settings'
import Toggle from '@/components/base/widget/Toggle.vue'

const { t } = useI18n()

const form = reactive<ToolSettings>({
  web_search: {
    enabled: false,
    use_builtin: true,
    api_key: '',
    base_url: 'https://api.kimi.com/coding/v1/search',
    proxy_enabled: false,
    proxy_addr: 'http://127.0.0.1:10808',
    max_results: 8,
    hide_search_results: false,
  },
  groups: {},
})

const status = reactive({ message: '', color: '#4ade80' })
const testing = ref(false)

const showStatus = (message: string, color = '#4ade80') => {
  status.message = message
  status.color = color
  setTimeout(() => {
    status.message = ''
  }, 5000)
}

const saveSettings = async () => {
  try {
    // 深拷贝一份普通对象，避免把 reactive 代理传给 Tauri IPC
    const payload: ToolSettings = JSON.parse(JSON.stringify(form))
    await saveToolSettings(payload)
    showStatus(t('ui.toolCalls.saveSuccess'))
  } catch (error: any) {
    showStatus(t('ui.toolCalls.saveFailed', { message: String(error) }), 'red')
  }
}

const runTest = async () => {
  if (testing.value) return
  testing.value = true
  try {
    // 测试前先保存，确保后端用的是页面上的最新配置
    await saveSettings()
    const result = await testWebSearch('LingChat')
    const parsed = JSON.parse(result)
    showStatus(t('ui.toolCalls.testSuccess', { count: parsed.result_count ?? 0 }))
  } catch (error: any) {
    showStatus(t('ui.toolCalls.testFailed', { message: String(error) }), 'red')
  } finally {
    testing.value = false
  }
}

onMounted(async () => {
  try {
    const settings = await getToolSettings()
    Object.assign(form.web_search, settings.web_search)
    Object.assign(form.groups, settings.groups ?? {})
  } catch (error) {
    console.error('加载工具配置失败:', error)
  }
})
</script>

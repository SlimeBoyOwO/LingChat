<template>
  <!-- 高级设置 → 工具配置：左侧导航列表 + 右侧内容 -->
  <div class="flex flex-col md:grid md:grid-cols-[min(28%,240px)_1fr] h-full min-h-0">
    <!-- 左侧导航 -->
    <nav class="flex flex-col gap-1 overflow-y-auto py-2 pr-0 md:pr-4 md:border-r border-brand/40">
      <a
        v-for="item in navItems"
        :key="item"
        href="#"
        class="block px-5 py-3 no-underline rounded-lg text-white transition-colors duration-200 relative z-10 adv-nav-link hover:bg-gray-200 hover:text-black active:text-white active:font-bold"
        :class="{ 'bg-brand/30 font-bold': selected === item }"
        @click.prevent="selected = item"
      >
        {{ navLabel(item) }}
      </a>
    </nav>

    <!-- 右侧内容 -->
    <main class="h-full overflow-y-auto custom-scrollbar px-2 md:px-6 py-2">
      <!-- ===== 网页搜索 ===== -->
      <div v-if="selected === 'web_search'">
        <h2 class="text-2xl text-brand font-semibold pb-4 mb-6 border-b border-brand">
          {{ $t('ui.toolCalls.webSearchTitle') }}
        </h2>

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

        <template v-if="!form.web_search.use_builtin">
          <label class="inline-flex items-center font-medium text-brand mt-2">
            {{ $t('ui.toolCalls.provider') }}
          </label>
          <select
            v-model="form.web_search.provider"
            class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200 cursor-pointer"
          >
            <option value="kimi" class="bg-slate-800 text-white">Kimi /search</option>
            <option value="bocha" class="bg-slate-800 text-white">BoCha 博查</option>
            <option value="custom" class="bg-slate-800 text-white">
              {{ $t('ui.toolCalls.providerCustom') }}
            </option>
          </select>

          <!-- 独立端点模式下 kimi/bocha/custom 后端都强制校验 API Key，始终显示输入框 -->
          <label class="inline-flex items-center font-medium text-brand mt-4">
            {{ $t('ui.toolCalls.apiKey') }}
          </label>
          <input
            type="password"
            v-model="form.web_search.api_key"
            :placeholder="$t('ui.toolCalls.apiKeyPlaceholder')"
            class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
          />

          <!-- 仅自定义端点需要填写地址；kimi/bocha 使用各自的固定端点 -->
          <template v-if="form.web_search.provider === 'custom'">

            <label class="inline-flex items-center font-medium text-brand mt-4">
              {{ $t('ui.toolCalls.baseUrl') }}
            </label>
            <p class="text-sm mt-1 mb-2 text-gray-300">
              {{ $t('ui.toolCalls.customHint') }}
            </p>
            <input
              type="text"
              v-model="form.web_search.base_url"
              placeholder="https://api.kimi.com/coding/v1/search"
              class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
            />
          </template>

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

        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.web_search.hide_search_results"
            @change="(value: boolean) => (form.web_search.hide_search_results = value)"
          />
          <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.hideSearchResults') }}</p>
        </div>

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
      </div>

      <!-- ===== 文件操作 ===== -->
      <div v-else-if="selected === 'file_ops'">
        <h2 class="text-2xl text-brand font-semibold pb-4 mb-6 border-b border-brand">
          {{ navLabel(selected) }}
        </h2>
        <p class="text-sm text-gray-400 mb-4 px-1">{{ $t('ui.toolCalls.otherToolsHint') }}</p>
        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.groups[selected] ?? false"
            @change="(value: boolean) => (form.groups[selected] = value)"
          />
          <p class="text-sm text-gray-300">{{ $t(`ui.toolCalls.groups.${selected}`) }}</p>
        </div>
        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.file_ops_allow_any_path"
            @change="(value: boolean) => (form.file_ops_allow_any_path = value)"
          />
          <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.fileOpsAllowAnyPath') }}</p>
        </div>
        <p v-if="form.file_ops_allow_any_path" class="text-sm text-amber-400 px-1 mb-2">
          {{ $t('ui.toolCalls.fileOpsAllowAnyPathHint') }}
        </p>
        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.file_delete_auto_approve"
            @change="(value: boolean) => (form.file_delete_auto_approve = value)"
          />
          <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.fileDeleteAutoApprove') }}</p>
        </div>
        <p v-if="form.file_delete_auto_approve" class="text-sm text-amber-400 px-1 mb-2">
          {{ $t('ui.toolCalls.fileDeleteAutoApproveHint') }}
        </p>
      </div>

      <!-- ===== 命令执行 ===== -->
      <div v-else-if="selected === 'command'">
        <h2 class="text-2xl text-brand font-semibold pb-4 mb-6 border-b border-brand">
          {{ navLabel(selected) }}
        </h2>
        <p class="text-sm text-gray-400 mb-4 px-1">{{ $t('ui.toolCalls.otherToolsHint') }}</p>
        <!-- 命令执行依赖本机 shell（cmd/sh），非 Windows 平台（如 Android）不可用 -->
        <p v-if="!isWindows()" class="text-sm text-amber-400 px-1 mb-2">
          {{ $t('ui.toolCalls.commandWindowsOnly') }}
        </p>
        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.groups[selected] ?? false"
            @change="(value: boolean) => (form.groups[selected] = value)"
          />
          <p class="text-sm text-gray-300">{{ $t(`ui.toolCalls.groups.${selected}`) }}</p>
        </div>
        <p class="text-sm text-gray-400 px-1 mb-2">{{ $t('ui.toolCalls.commandHint') }}</p>
        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.command_auto_approve"
            @change="(value: boolean) => (form.command_auto_approve = value)"
          />
          <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.commandAutoApprove') }}</p>
        </div>
        <p v-if="form.command_auto_approve" class="text-sm text-amber-400 px-1 mb-2">
          {{ $t('ui.toolCalls.commandAutoApproveHint') }}
        </p>
        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.command_delete_auto_approve"
            @change="(value: boolean) => (form.command_delete_auto_approve = value)"
          />
          <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.commandDeleteAutoApprove') }}</p>
        </div>
        <p v-if="form.command_delete_auto_approve" class="text-sm text-amber-400 px-1 mb-2">
          {{ $t('ui.toolCalls.commandDeleteAutoApproveHint') }}
        </p>
      </div>

      <!-- ===== 其他工具组 ===== -->
      <div v-else>
        <h2 class="text-2xl text-brand font-semibold pb-4 mb-6 border-b border-brand">
          {{ navLabel(selected) }}
        </h2>
        <p class="text-sm text-gray-400 mb-4 px-1">{{ $t('ui.toolCalls.otherToolsHint') }}</p>
        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.groups[selected] ?? false"
            @change="(value: boolean) => (form.groups[selected] = value)"
          />
          <p class="text-sm text-gray-300">{{ $t(`ui.toolCalls.groups.${selected}`) }}</p>
        </div>
      </div>

      <!-- 保存/测试操作区 -->
      <div class="flex gap-2 items-center mt-6">
        <div
          class="w-18 px-5 py-2.5 bg-brand text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#0056b3]"
          @click="saveSettings"
        >
          {{ $t('ui.toolCalls.save') }}
        </div>
        <div
          v-if="selected === 'web_search'"
          class="px-5 py-2.5 bg-white/10 text-white border border-white/20 rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-white/20"
          @click="runTest"
        >
          {{ $t('ui.toolCalls.test') }}
        </div>
        <p class="text-sm" :style="{ color: status.color }">{{ status.message }}</p>
      </div>
    </main>
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
import { isWindows } from '@/utils/platform'

const { t, te } = useI18n()

/** 当前选中的设置项：'web_search' 或工具组名 */
const selected = ref<string>('web_search')

const navItems = ['web_search', ...TOOL_GROUP_KEYS] as const

const navLabel = (item: string) =>
  item === 'web_search'
    ? t('ui.toolCalls.webSearchTitle')
    : te(`ui.toolCalls.nav.${item}`)
      ? t(`ui.toolCalls.nav.${item}`)
      : t(`ui.toolCalls.groups.${item}`)

const form = reactive<ToolSettings>({
  web_search: {
    enabled: false,
    use_builtin: true,
    provider: 'kimi',
    api_key: '',
    base_url: '',
    proxy_enabled: false,
    proxy_addr: 'http://127.0.0.1:10808',
    max_results: 8,
    hide_search_results: false,
  },
  groups: {},
  command_auto_approve: false,
  command_delete_auto_approve: false,
  file_delete_auto_approve: false,
  file_ops_allow_any_path: false,
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
    form.command_auto_approve = settings.command_auto_approve ?? false
    form.command_delete_auto_approve = settings.command_delete_auto_approve ?? false
    form.file_delete_auto_approve = settings.file_delete_auto_approve ?? false
    form.file_ops_allow_any_path = settings.file_ops_allow_any_path ?? false
  } catch (error) {
    console.error('加载工具配置失败:', error)
  }
})
</script>

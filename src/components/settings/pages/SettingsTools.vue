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

      <!-- ===== 场景背景生成（NovelAI） ===== -->
      <div v-else-if="selected === 'image_gen'">
        <h2 class="text-2xl text-brand font-semibold pb-4 mb-6 border-b border-brand">
          {{ $t('ui.toolCalls.imageGenTitle') }}
        </h2>

        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.image_gen.enabled"
            @change="(value: boolean) => (form.image_gen.enabled = value)"
          />
          <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.enableImageGen') }}</p>
        </div>
        <p class="text-sm text-gray-400 px-1 mb-2">{{ $t('ui.toolCalls.imageGenHint') }}</p>

        <label class="inline-flex items-center font-medium text-brand mt-4">
          {{ $t('ui.toolCalls.naiToken') }}
        </label>
        <p class="text-sm mt-1 mb-2 text-gray-300">{{ $t('ui.toolCalls.naiTokenHint') }}</p>
        <input
          type="password"
          v-model="form.image_gen.api_token"
          :placeholder="$t('ui.toolCalls.naiTokenPlaceholder')"
          class="w-full mt-1 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
        />

        <label class="inline-flex items-center font-medium text-brand mt-4">
          {{ $t('ui.toolCalls.naiModel') }}
        </label>
        <select
          v-model="form.image_gen.model"
          class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200 cursor-pointer"
        >
          <option value="nai-diffusion-4-5-full" class="bg-slate-800 text-white">
            NAI Diffusion V4.5 Full
          </option>
          <option value="nai-diffusion-4-5-curated" class="bg-slate-800 text-white">
            NAI Diffusion V4.5 Curated
          </option>
          <option value="nai-diffusion-4-full" class="bg-slate-800 text-white">
            NAI Diffusion V4 Full
          </option>
          <!-- 服务端只认 -curated-preview 这个拼写，nai-diffusion-4-curated 会被拒 -->
          <option value="nai-diffusion-4-curated-preview" class="bg-slate-800 text-white">
            NAI Diffusion V4 Curated
          </option>
          <option value="nai-diffusion-3" class="bg-slate-800 text-white">
            NAI Diffusion V3
          </option>
        </select>

        <div class="grid grid-cols-3 gap-3 mt-4">
          <div>
            <label class="inline-flex items-center font-medium text-brand text-sm">
              {{ $t('ui.toolCalls.naiWidth') }}
            </label>
            <input
              type="number"
              v-model.number="form.image_gen.width"
              min="64"
              max="1600"
              step="64"
              class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
            />
          </div>
          <div>
            <label class="inline-flex items-center font-medium text-brand text-sm">
              {{ $t('ui.toolCalls.naiHeight') }}
            </label>
            <input
              type="number"
              v-model.number="form.image_gen.height"
              min="64"
              max="1600"
              step="64"
              class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
            />
          </div>
          <div>
            <label class="inline-flex items-center font-medium text-brand text-sm">
              {{ $t('ui.toolCalls.naiSteps') }}
            </label>
            <input
              type="number"
              v-model.number="form.image_gen.steps"
              min="1"
              max="50"
              step="1"
              class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
            />
          </div>
        </div>

        <!-- 免费额度实时提示：越线时用告警色，因为越线就开始扣 Anlas -->
        <p class="text-sm px-1 mt-2" :class="withinFreeTier ? 'text-gray-400' : 'text-amber-400'">
          {{
            withinFreeTier
              ? $t('ui.toolCalls.freeTierOk', { pixels: currentPixels.toLocaleString() })
              : $t('ui.toolCalls.freeTierExceeded', {
                  pixels: currentPixels.toLocaleString(),
                  maxPixels: NAI_FREE_MAX_PIXELS.toLocaleString(),
                  maxSteps: NAI_FREE_MAX_STEPS,
                })
          }}
        </p>

        <label class="inline-flex items-center font-medium text-brand mt-4">
          {{ $t('ui.toolCalls.stylePrompt') }}
        </label>
        <p class="text-sm mt-1 mb-2 text-gray-300">{{ $t('ui.toolCalls.stylePromptHint') }}</p>
        <input
          type="text"
          v-model="form.image_gen.style_prompt"
          placeholder="no humans, scenery, detailed background"
          class="w-full mt-1 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
        />

        <label class="inline-flex items-center font-medium text-brand mt-4">
          {{ $t('ui.toolCalls.naiNegativePrompt') }}
        </label>
        <input
          type="text"
          v-model="form.image_gen.negative_prompt"
          placeholder="1girl, 1boy, person, character focus"
          class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
        />

        <div class="flex items-center gap-3 py-2.5 px-1 mt-4">
          <Toggle
            :checked="form.image_gen.require_confirm"
            @change="(value: boolean) => (form.image_gen.require_confirm = value)"
          />
          <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.requireConfirm') }}</p>
        </div>
        <p v-if="!form.image_gen.require_confirm" class="text-sm text-amber-400 px-1 mb-2">
          {{ $t('ui.toolCalls.requireConfirmOffHint') }}
        </p>

        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.image_gen.free_tier_only"
            @change="(value: boolean) => (form.image_gen.free_tier_only = value)"
          />
          <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.freeTierOnly') }}</p>
        </div>
        <p v-if="!form.image_gen.free_tier_only" class="text-sm text-amber-400 px-1 mb-2">
          {{ $t('ui.toolCalls.freeTierOnlyOffHint') }}
        </p>

        <div class="flex items-center gap-3 py-2.5 px-1 mt-2">
          <Toggle
            :checked="form.image_gen.proxy_enabled"
            @change="(value: boolean) => (form.image_gen.proxy_enabled = value)"
          />
          <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.proxyEnable') }}</p>
        </div>
        <input
          v-if="form.image_gen.proxy_enabled"
          type="text"
          v-model="form.image_gen.proxy_addr"
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
        <div
          v-if="selected === 'image_gen'"
          class="px-5 py-2.5 bg-white/10 text-white border border-white/20 rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-white/20"
          @click="runConnectionTest"
        >
          {{ $t('ui.toolCalls.testConnection') }}
        </div>
        <p class="text-sm" :style="{ color: status.color }">{{ status.message }}</p>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  getToolSettings,
  saveToolSettings,
  testWebSearch,
  TOOL_GROUP_KEYS,
  NAI_FREE_MAX_PIXELS,
  NAI_FREE_MAX_STEPS,
  type ToolSettings,
} from '@/api/services/tool-settings'
import { testNovelaiConnection } from '@/api/services/scene'
import Toggle from '@/components/base/widget/Toggle.vue'
import { isWindows } from '@/utils/platform'

const { t, te } = useI18n()

/** 当前选中的设置项：'web_search'、'image_gen' 或工具组名 */
const selected = ref<string>('web_search')

const navItems = ['web_search', 'image_gen', ...TOOL_GROUP_KEYS] as const

const navLabel = (item: string) => {
  if (item === 'web_search') return t('ui.toolCalls.webSearchTitle')
  if (item === 'image_gen') return t('ui.toolCalls.imageGenTitle')
  return te(`ui.toolCalls.nav.${item}`)
    ? t(`ui.toolCalls.nav.${item}`)
    : t(`ui.toolCalls.groups.${item}`)
}

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
  image_gen: {
    enabled: false,
    api_token: '',
    base_url: 'https://image.novelai.net',
    model: 'nai-diffusion-4-5-full',
    width: 1216,
    height: 832,
    steps: 23,
    scale: 5.0,
    sampler: 'k_euler_ancestral',
    noise_schedule: 'karras',
    uc_preset: 'light',
    quality_toggle: true,
    style_prompt: 'no humans, scenery, detailed background',
    negative_prompt: '1girl, 1boy, person, character focus',
    require_confirm: false,
    free_tier_only: true,
    proxy_enabled: false,
    proxy_addr: 'http://127.0.0.1:10808',
  },
  groups: {},
  command_auto_approve: false,
  command_delete_auto_approve: false,
  file_delete_auto_approve: false,
  file_ops_allow_any_path: false,
})

const status = reactive({ message: '', color: '#4ade80' })
const testing = ref(false)

/** 当前尺寸的像素数与免费额度判定（与后端 check_free_tier 同一套规则）。 */
const currentPixels = computed(() => form.image_gen.width * form.image_gen.height)
const withinFreeTier = computed(
  () => currentPixels.value <= NAI_FREE_MAX_PIXELS && form.image_gen.steps <= NAI_FREE_MAX_STEPS,
)

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

const runConnectionTest = async () => {
  if (testing.value) return
  testing.value = true
  try {
    // 先保存，确保后端用的是页面上刚填的 Token
    await saveSettings()
    const info = await testNovelaiConnection()
    showStatus(
      t('ui.toolCalls.testConnectionSuccess', {
        tier: info.is_opus ? 'Opus' : `Tier ${info.tier}`,
        anlas: info.anlas,
      }),
    )
  } catch (error: any) {
    showStatus(t('ui.toolCalls.testConnectionFailed', { message: String(error) }), 'red')
  } finally {
    testing.value = false
  }
}

onMounted(async () => {
  try {
    const settings = await getToolSettings()
    Object.assign(form.web_search, settings.web_search)
    if (settings.image_gen) Object.assign(form.image_gen, settings.image_gen)
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

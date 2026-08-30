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
      <div
        v-if="android"
        class="mb-5 rounded-xl border border-sky-300/30 bg-sky-400/10 px-4 py-3 text-sm"
      >
        <p class="font-semibold text-sky-200">{{ $t('ui.toolCalls.androidTitle') }}</p>
        <p class="mt-1 text-gray-300">{{ $t('ui.toolCalls.androidSummary') }}</p>
        <p v-if="runtimeInfo && !runtimeInfo.modelConfigured" class="mt-2 text-amber-300">
          {{ $t('ui.toolCalls.androidNoModel') }}
        </p>
        <p
          v-else-if="runtimeInfo && !runtimeInfo.nativeToolCallsSupported"
          class="mt-2 text-amber-300"
        >
          {{ $t('ui.toolCalls.androidModelUnsupported') }}
        </p>
        <p v-else-if="runtimeInfo && runtimeInfo.allowedTools.length === 0" class="mt-2 text-amber-300">
          {{ $t('ui.toolCalls.androidNoTools') }}
        </p>
        <p v-else-if="runtimeInfo" class="mt-2 text-emerald-300">
          {{ $t('ui.toolCalls.androidReady', { count: runtimeInfo.allowedTools.length }) }}
        </p>
        <button
          type="button"
          class="mt-3 rounded-lg border border-sky-200/30 bg-sky-300/15 px-3 py-2 text-sky-100 transition-colors hover:bg-sky-300/25"
          @click="enableAndroidRecommended"
        >
          {{ $t('ui.toolCalls.androidEnableRecommended') }}
        </button>
      </div>

      <!-- ===== 工具访问模式 ===== -->
      <div v-if="selected === 'access'">
        <h2 class="text-2xl text-brand font-semibold pb-4 mb-6 border-b border-brand">
          {{ $t('ui.toolCalls.accessModeTitle') }}
        </h2>
        <p class="text-sm text-gray-300 mb-4 px-1">
          {{ $t('ui.toolCalls.accessModeHint') }}
        </p>
        <div class="grid gap-3">
          <button
            v-for="mode in accessModes"
            :key="mode"
            type="button"
            class="w-full rounded-xl border px-4 py-3 text-left transition-all duration-200"
            :class="[
              form.access_mode === mode
                ? mode === 'full_access'
                  ? 'border-amber-400/80 bg-amber-400/10 ring-1 ring-amber-400/25'
                  : 'border-brand bg-brand/15 ring-1 ring-brand/25'
                : 'border-white/15 bg-white/5 hover:bg-white/10',
            ]"
            @click="selectAccessMode(mode)"
          >
            <div class="flex items-center justify-between gap-3">
              <span class="flex items-center gap-2.5">
                <!-- 单选圆点：选中态填充，未选中仅描边 -->
                <span
                  class="flex h-4 w-4 shrink-0 items-center justify-center rounded-full border transition-colors duration-200"
                  :class="
                    form.access_mode === mode
                      ? mode === 'full_access'
                        ? 'border-amber-400'
                        : 'border-brand'
                      : 'border-white/30'
                  "
                >
                  <span
                    v-if="form.access_mode === mode"
                    class="h-2 w-2 rounded-full"
                    :class="mode === 'full_access' ? 'bg-amber-400' : 'bg-brand'"
                  ></span>
                </span>
                <span
                  class="font-semibold"
                  :class="mode === 'full_access' ? 'text-amber-300' : 'text-white'"
                >
                  {{ $t(`ui.toolCalls.accessModes.${mode}.label`) }}
                </span>
              </span>
              <span
                v-if="form.access_mode === mode"
                class="rounded-full border px-2 py-0.5 text-xs"
                :class="
                  mode === 'full_access'
                    ? 'border-amber-400/60 text-amber-300'
                    : 'border-brand/60 text-brand'
                "
              >
                {{ $t('ui.toolCalls.accessModeSelected') }}
              </span>
            </div>
            <p class="mt-1 text-sm text-gray-300">
              {{ $t(`ui.toolCalls.accessModes.${mode}.description`) }}
            </p>
          </button>
        </div>
        <div class="mt-3 rounded-xl border border-white/15 bg-white/5 px-4 py-3">
          <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div class="min-w-0">
              <h3 class="font-semibold text-white">
                {{ $t('ui.toolCalls.maxToolRoundsTitle') }}
              </h3>
              <p class="mt-1 text-sm leading-relaxed text-gray-300">
                {{ $t('ui.toolCalls.maxToolRoundsHint') }}
              </p>
            </div>
            <div class="flex shrink-0 flex-col items-start gap-1 sm:items-end">
              <label
                class="flex items-center gap-2 rounded-lg border border-white/20 bg-black/20 px-3 py-2 focus-within:border-brand"
              >
                <!-- 隐藏原生数字箭头，避免与深色玻璃风格冲突 -->
                <input
                  v-model.number="form.max_tool_rounds"
                  type="number"
                  :min="MIN_TOOL_ROUND_LIMIT"
                  :max="MAX_TOOL_ROUND_LIMIT"
                  step="1"
                  class="w-16 bg-transparent text-center font-semibold text-white outline-none [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
                  :aria-label="$t('ui.toolCalls.maxToolRoundsTitle')"
                  @blur="normalizeToolRoundLimit"
                />
                <span class="text-sm text-gray-300">{{ $t('ui.toolCalls.maxToolRoundsUnit') }}</span>
              </label>
              <p class="px-1 text-xs text-gray-400">
                {{
                  $t('ui.toolCalls.maxToolRoundsRange', {
                    min: MIN_TOOL_ROUND_LIMIT,
                    max: MAX_TOOL_ROUND_LIMIT,
                    default: DEFAULT_TOOL_ROUND_LIMIT,
                  })
                }}
              </p>
            </div>
          </div>
        </div>
        <div
          v-if="form.access_mode === 'full_access'"
          class="mt-3 rounded-lg border-l-4 border-amber-400/70 bg-amber-400/10 px-4 py-3 text-sm text-amber-200/90"
        >
          {{ $t('ui.toolCalls.fullAccessSettingsWarning') }}
        </div>
        <div
          v-if="form.access_mode === 'full_access' && isWindows()"
          class="mt-3 rounded-xl border border-white/15 bg-black/20 px-4 py-3"
        >
          <div class="flex items-center gap-2 font-semibold text-white">
            <span
              class="h-2.5 w-2.5 rounded-full"
              :class="elevationStatus === 'elevated' ? 'bg-emerald-400' : 'bg-amber-400'"
            ></span>
            {{ $t('ui.toolCalls.adminModeTitle') }}
          </div>
          <p class="mt-2 text-sm text-gray-300">
            {{
              $t(
                elevationStatus === 'elevated'
                  ? 'ui.toolCalls.adminModeElevated'
                  : elevationStatus === 'checking'
                    ? 'ui.toolCalls.adminModeChecking'
                    : 'ui.toolCalls.adminModeStandard',
              )
            }}
          </p>
          <button
            v-if="elevationStatus !== 'elevated'"
            type="button"
            class="mt-3 rounded-lg border border-amber-400/60 bg-amber-400/15 px-4 py-2 text-sm font-semibold text-amber-200 transition-colors hover:bg-amber-400/25 disabled:cursor-wait disabled:opacity-60"
            :disabled="elevationRestarting || elevationStatus === 'checking'"
            @click="restartAsAdmin"
          >
            {{
              $t(
                elevationRestarting
                  ? 'ui.toolCalls.adminModeRestarting'
                  : 'ui.toolCalls.adminModeRestart',
              )
            }}
          </button>
          <p v-if="elevationStatus !== 'elevated'" class="mt-2 text-xs text-gray-400">
            {{ $t('ui.toolCalls.adminModeHint') }}
          </p>
        </div>
      </div>

      <!-- ===== 网页搜索 ===== -->
      <div v-else-if="selected === 'web_search'">
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

        <label class="inline-flex items-center font-medium text-brand mt-2">
          {{ $t('ui.toolCalls.provider') }}
        </label>
          <select
            v-model="form.web_search.provider"
            class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200 cursor-pointer"
          >
            <option value="kimi" class="bg-slate-800 text-white">Kimi /search</option>
            <option value="bocha" class="bg-slate-800 text-white">BoCha 博查</option>
            <option value="deepseek" class="bg-slate-800 text-white">
              {{ $t('ui.toolCalls.providerDeepSeek') }}
            </option>
            <option value="tavily" class="bg-slate-800 text-white">Tavily</option>
            <option value="codex" class="bg-slate-800 text-white">
              {{ $t('ui.toolCalls.providerCodex') }}
            </option>
            <option value="custom" class="bg-slate-800 text-white">
              {{ $t('ui.toolCalls.providerCustom') }}
            </option>
          </select>

          <!-- Codex：复用已登录的订阅凭据，无需 API Key -->
          <template v-if="form.web_search.provider === 'codex'">
            <label class="inline-flex items-center font-medium text-brand mt-4">
              {{ $t('ui.toolCalls.apiKey') }}
            </label>
            <p class="text-sm mt-2 mb-2 text-gray-300">
              {{ $t('ui.toolCalls.codexHint') }}
            </p>
          </template>

          <!-- Kimi 可留空复用当前官方 Kimi Code 对话凭据；其他独立端点需要 API Key -->
          <template v-else>
            <label class="inline-flex items-center font-medium text-brand mt-4">
              {{ $t('ui.toolCalls.apiKey') }}
            </label>
            <input
              type="password"
              v-model="form.web_search.api_key"
              :placeholder="
                form.web_search.provider === 'deepseek'
                  ? $t('ui.toolCalls.dsApiKeyPlaceholder')
                  : form.web_search.provider === 'kimi'
                    ? $t('ui.toolCalls.kimiApiKeyPlaceholder')
                    : $t('ui.toolCalls.apiKeyPlaceholder')
              "
              class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
            />
            <p
              v-if="form.web_search.provider === 'kimi'"
              class="text-sm mt-2 mb-2 text-gray-300"
            >
              {{ $t('ui.toolCalls.kimiHint') }}
            </p>
          </template>

          <!-- DeepSeek Responses：可切换模型；结果数量由服务端决定，不展示条数设置 -->
          <template v-if="form.web_search.provider === 'deepseek'">
            <label class="inline-flex items-center font-medium text-brand mt-4">
              {{ $t('ui.toolCalls.dsModel') }}
            </label>
            <input
              type="text"
              v-model="form.web_search.model"
              placeholder="deepseek-v4-flash"
              class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
            />
            <p class="text-sm mt-2 mb-2 text-gray-300">
              {{ $t('ui.toolCalls.dsHint') }}
            </p>
          </template>

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

          <template v-if="form.web_search.provider !== 'deepseek'">
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
        <p v-if="android" class="text-sm text-amber-300 px-1 mb-2">
          {{ $t('ui.toolCalls.androidProxyHint') }}
        </p>
        <input
          v-if="form.web_search.proxy_enabled"
          type="text"
          v-model="form.web_search.proxy_addr"
          placeholder="http://127.0.0.1:10808"
          class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
        />
      </div>

      <!-- ===== 图片/视频识别 ===== -->
      <div v-else-if="selected === 'media'">
        <h2 class="text-2xl text-brand font-semibold pb-4 mb-6 border-b border-brand">
          {{ $t('ui.toolCalls.mediaTitle') }}
        </h2>
        <p class="text-sm text-gray-300 mb-4 px-1">
          {{ $t('ui.toolCalls.mediaHint') }}
        </p>

        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.groups.media ?? false"
            @change="(value: boolean) => (form.groups.media = value)"
          />
          <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.mediaEnable') }}</p>
        </div>

        <div class="my-3 rounded-xl border border-brand/30 bg-brand/10 px-4 py-3">
          <p class="text-sm text-gray-200">{{ $t('ui.toolCalls.mediaVisionModelHint') }}</p>
        </div>

        <div class="grid gap-2 sm:grid-cols-2">
          <div class="flex items-center gap-3 rounded-lg bg-white/5 px-3 py-2.5">
            <Toggle
              :checked="form.media_file.image_enabled"
              @change="(value: boolean) => (form.media_file.image_enabled = value)"
            />
            <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.mediaImages') }}</p>
          </div>
          <div class="flex items-center gap-3 rounded-lg bg-white/5 px-3 py-2.5">
            <Toggle
              :checked="form.media_file.video_enabled"
              @change="(value: boolean) => (form.media_file.video_enabled = value)"
            />
            <p class="text-sm text-gray-300">{{ $t('ui.toolCalls.mediaVideos') }}</p>
          </div>
        </div>

        <div class="mt-4 grid gap-4 sm:grid-cols-2">
          <label class="block">
            <span class="text-sm font-medium text-brand">{{ $t('ui.toolCalls.mediaMaxFileMb') }}</span>
            <input
              v-model.number="form.media_file.max_file_mb"
              type="number"
              min="1"
              max="100"
              step="1"
              class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 border-white/10 focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20"
            />
          </label>
          <label class="block">
            <span class="text-sm font-medium text-brand">{{ $t('ui.toolCalls.mediaOutputTokens') }}</span>
            <input
              v-model.number="form.media_file.max_output_tokens"
              type="number"
              min="128"
              max="4096"
              step="128"
              class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 border-white/10 focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20"
            />
          </label>
          <label class="block">
            <span class="text-sm font-medium text-brand">{{ $t('ui.toolCalls.mediaImageMaxEdge') }}</span>
            <input
              v-model.number="form.media_file.image_max_edge"
              type="number"
              min="512"
              max="4096"
              step="128"
              :disabled="!form.media_file.image_enabled"
              class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 border-white/10 focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 disabled:opacity-50"
            />
          </label>
          <label class="block">
            <span class="text-sm font-medium text-brand">{{ $t('ui.toolCalls.mediaJpegQuality') }}</span>
            <input
              v-model.number="form.media_file.jpeg_quality"
              type="number"
              min="50"
              max="95"
              step="1"
              :disabled="!form.media_file.image_enabled"
              class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 border-white/10 focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 disabled:opacity-50"
            />
          </label>
        </div>

        <label class="block mt-4">
          <span class="text-sm font-medium text-brand">{{ $t('ui.toolCalls.mediaDefaultPrompt') }}</span>
          <textarea
            v-model="form.media_file.default_prompt"
            rows="3"
            class="w-full mt-2 px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 border-white/10 focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 resize-y"
          ></textarea>
        </label>

        <p v-if="form.media_file.video_enabled" class="mt-3 px-1 text-sm text-amber-300/90">
          {{ $t('ui.toolCalls.mediaVideoCompatibility') }}
        </p>
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
        <p class="text-sm px-1 mb-2" :class="form.access_mode === 'full_access' ? 'text-amber-300' : 'text-gray-400'">
          {{ $t(`ui.toolCalls.fileAccessByMode.${form.access_mode}`) }}
        </p>
        <p v-if="android" class="text-sm text-amber-300 px-1 mb-2">
          {{ $t('ui.toolCalls.androidFileScope') }}
        </p>
      </div>

      <!-- ===== 命令执行 ===== -->
      <div v-else-if="selected === 'command'">
        <h2 class="text-2xl text-brand font-semibold pb-4 mb-6 border-b border-brand">
          {{ navLabel(selected) }}
        </h2>
        <p class="text-sm text-gray-400 mb-4 px-1">{{ $t('ui.toolCalls.otherToolsHint') }}</p>
        <p v-if="!commandAvailable" class="text-sm text-amber-400 px-1 mb-2">
          {{ $t('ui.toolCalls.commandWindowsOnly') }}
        </p>
        <div class="flex items-center gap-3 py-2.5 px-1">
          <Toggle
            :checked="form.groups[selected] ?? false"
            :disabled="!commandAvailable"
            @change="(value: boolean) => (form.groups[selected] = value)"
          />
          <p class="text-sm text-gray-300">{{ $t(`ui.toolCalls.groups.${selected}`) }}</p>
        </div>
        <p class="text-sm px-1 mb-2" :class="form.access_mode === 'full_access' ? 'text-amber-300' : 'text-gray-400'">
          {{ $t(`ui.toolCalls.commandAccessByMode.${form.access_mode}`) }}
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
import { computed, reactive, ref, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  getToolSettings,
  getToolElevationStatus,
  getToolRuntimeInfo,
  restartToolProcessAsAdmin,
  saveToolSettings,
  testWebSearch,
  TOOL_GROUP_KEYS,
  type ToolAccessMode,
  type ToolRuntimeInfo,
  type ToolSettings,
} from '@/api/services/tool-settings'
import Toggle from '@/components/base/widget/Toggle.vue'
import { isAndroid, isWindows } from '@/utils/platform'
import { useDialogStore } from '@/stores/modules/ui/dialog'

const { t, te } = useI18n()
const dialogStore = useDialogStore()

/** 当前选中的设置项：访问模式、web_search 或工具组名 */
const selected = ref<string>('access')
const android = isAndroid()
const runtimeInfo = ref<ToolRuntimeInfo | null>(null)
const commandAvailable = computed(() => runtimeInfo.value?.commandAvailable ?? !android)

const navItems = ['access', 'web_search', ...TOOL_GROUP_KEYS] as const
const accessModes: ToolAccessMode[] = ['manual', 'auto_approve', 'full_access']
const DEFAULT_TOOL_ROUND_LIMIT = 8
const MIN_TOOL_ROUND_LIMIT = 1
const MAX_TOOL_ROUND_LIMIT = 64
const DEFAULT_MEDIA_PROMPT =
  '请详细识别并描述这个媒体文件的内容；如果其中包含文字、界面、人物、物体、动作或时间顺序，请准确说明。'

const navLabel = (item: string) =>
  item === 'access'
    ? t('ui.toolCalls.accessModeTitle')
    : item === 'web_search'
    ? t('ui.toolCalls.webSearchTitle')
    : te(`ui.toolCalls.nav.${item}`)
      ? t(`ui.toolCalls.nav.${item}`)
      : t(`ui.toolCalls.groups.${item}`)

const form = reactive<ToolSettings>({
  web_search: {
    enabled: false,
    provider: 'kimi',
    model: 'deepseek-v4-flash',
    api_key: '',
    base_url: '',
    proxy_enabled: false,
    proxy_addr: 'http://127.0.0.1:10808',
    max_results: 8,
    hide_search_results: false,
  },
  media_file: {
    image_enabled: true,
    video_enabled: true,
    max_file_mb: 100,
    image_max_edge: 2000,
    jpeg_quality: 85,
    max_output_tokens: 1024,
    default_prompt: DEFAULT_MEDIA_PROMPT,
  },
  groups: {},
  access_mode: 'manual',
  max_tool_rounds: DEFAULT_TOOL_ROUND_LIMIT,
})

const status = reactive({ message: '', color: '#4ade80' })
const testing = ref(false)
const elevationStatus = ref<'checking' | 'standard' | 'elevated'>('checking')
const elevationRestarting = ref(false)

const loadRuntimeInfo = async () => {
  try {
    runtimeInfo.value = await getToolRuntimeInfo()
  } catch (error) {
    console.warn('加载工具运行状态失败:', error)
  }
}

const enableAndroidRecommended = () => {
  for (const group of TOOL_GROUP_KEYS) {
    form.groups[group] = group !== 'command'
  }
  showStatus(t('ui.toolCalls.androidRecommendedStaged'), '#7dd3fc')
}

const showStatus = (message: string, color = '#4ade80') => {
  status.message = message
  status.color = color
  setTimeout(() => {
    status.message = ''
  }, 5000)
}

const selectAccessMode = async (mode: ToolAccessMode) => {
  if (mode === form.access_mode) return
  if (mode === 'full_access') {
    const approved = await dialogStore.confirm(
      t('ui.toolCalls.fullAccessConfirmMessage'),
      t('ui.toolCalls.fullAccessConfirmTitle'),
    )
    if (!approved) return
  }
  form.access_mode = mode
}

const normalizeToolRoundLimit = () => {
  const value = Number(form.max_tool_rounds)
  form.max_tool_rounds = Number.isFinite(value)
    ? Math.min(MAX_TOOL_ROUND_LIMIT, Math.max(MIN_TOOL_ROUND_LIMIT, Math.round(value)))
    : DEFAULT_TOOL_ROUND_LIMIT
}

const normalizeMediaSettings = () => {
  const clamp = (value: unknown, fallback: number, min: number, max: number) => {
    const parsed = Number(value)
    return Number.isFinite(parsed) ? Math.min(max, Math.max(min, Math.round(parsed))) : fallback
  }
  form.media_file.max_file_mb = clamp(form.media_file.max_file_mb, 100, 1, 100)
  form.media_file.image_max_edge = clamp(form.media_file.image_max_edge, 2000, 512, 4096)
  form.media_file.jpeg_quality = clamp(form.media_file.jpeg_quality, 85, 50, 95)
  form.media_file.max_output_tokens = clamp(form.media_file.max_output_tokens, 1024, 128, 4096)
  if (!form.media_file.default_prompt.trim()) form.media_file.default_prompt = DEFAULT_MEDIA_PROMPT
}

const saveSettings = async (): Promise<boolean> => {
  try {
    normalizeToolRoundLimit()
    normalizeMediaSettings()
    if (android) {
      form.groups.command = false
    }
    // 深拷贝一份普通对象，避免把 reactive 代理传给 Tauri IPC
    const payload: ToolSettings = JSON.parse(JSON.stringify(form))
    // deepseek 使用官方 /responses 端点；base_url 对该 provider 不可编辑，
    // 清空避免把 kimi 的默认端点残留进配置导致请求打到错误地址
    if (payload.web_search.provider === 'deepseek') {
      payload.web_search.base_url = ''
    }
    await saveToolSettings(payload)
    await loadRuntimeInfo()
    showStatus(t('ui.toolCalls.saveSuccess'))
    return true
  } catch (error: any) {
    showStatus(t('ui.toolCalls.saveFailed', { message: String(error) }), 'red')
    return false
  }
}

const refreshElevationStatus = async () => {
  elevationStatus.value = 'checking'
  try {
    elevationStatus.value = (await getToolElevationStatus()) ? 'elevated' : 'standard'
  } catch (error) {
    console.warn('读取管理员权限状态失败:', error)
    elevationStatus.value = 'standard'
  }
}

const restartAsAdmin = async () => {
  if (elevationRestarting.value) return
  const approved = await dialogStore.confirm(
    t('ui.toolCalls.adminModeConfirmMessage'),
    t('ui.toolCalls.adminModeConfirmTitle'),
  )
  if (!approved) return
  if (!(await saveSettings())) return
  elevationRestarting.value = true
  try {
    await restartToolProcessAsAdmin()
  } catch (error: any) {
    elevationRestarting.value = false
    showStatus(t('ui.toolCalls.adminModeRestartFailed', { message: String(error) }), 'red')
  }
}

// 切换到 deepseek provider 时同步清空 base_url（加载旧配置时同样生效）
watch(
  () => form.web_search.provider,
  (provider) => {
    if (provider === 'deepseek') {
      form.web_search.base_url = ''
    }
  },
)

const runTest = async () => {
  if (testing.value) return
  testing.value = true
  try {
    // 测试前先保存，确保后端用的是页面上的最新配置
    if (!(await saveSettings())) return
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
    Object.assign(form.media_file, settings.media_file ?? {})
    Object.assign(form.groups, settings.groups ?? {})
    form.access_mode = settings.access_mode ?? 'manual'
    form.max_tool_rounds = settings.max_tool_rounds ?? DEFAULT_TOOL_ROUND_LIMIT
    normalizeToolRoundLimit()
    normalizeMediaSettings()
    await loadRuntimeInfo()
    if (isWindows()) await refreshElevationStatus()
  } catch (error) {
    console.error('加载工具配置失败:', error)
  }
})
</script>

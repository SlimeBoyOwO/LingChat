<template>
  <div class="settings-asr p-4">
    <header class="flex items-center justify-between mb-4">
      <h2 class="text-xl font-semibold">{{ t('settings.asr.title') }}</h2>
      <span class="text-sm" :class="statusClass">{{ statusText }}</span>
    </header>

    <section class="mb-6">
      <label class="flex items-center justify-between">
        <div>
          <div>{{ t('settings.asr.autoListen') }}</div>
          <div class="text-sm text-gray-500">{{ t('settings.asr.autoListenHint') }}</div>
        </div>
        <input
          type="checkbox"
          v-model="localSettings.auto_listen"
          class="w-5 h-5"
        />
      </label>
    </section>

    <section class="mb-6">
      <label class="flex items-center justify-between">
        <div>{{ t('settings.asr.hotkey.enable') }}</div>
        <input
          type="checkbox"
          v-model="localSettings.hotkey_enabled"
          class="w-5 h-5"
        />
      </label>
      <div v-if="localSettings.hotkey_enabled" class="mt-2 flex items-center gap-2">
        <span class="text-sm text-gray-600">
          {{ t('settings.asr.hotkey.combination') }}:
          <kbd class="px-2 py-1 bg-gray-100 rounded">{{ localSettings.hotkey_combination }}</kbd>
        </span>
        <button
          class="px-3 py-1 text-sm bg-blue-500 text-white rounded hover:bg-blue-600"
          @click="recordHotkey"
        >
          {{ t('settings.asr.hotkey.record') }}
        </button>
      </div>
    </section>

    <section class="mb-6">
      <div class="font-medium mb-2">{{ t('settings.asr.sendMode.title') }}</div>
      <label
        v-for="opt in sendModeOptions"
        :key="opt.value"
        class="flex items-center gap-2 mb-1"
      >
        <input
          type="radio"
          :value="opt.value"
          v-model="localSettings.send_mode"
          class="w-4 h-4"
        />
        <span>{{ opt.label }}</span>
      </label>
    </section>

    <section class="mb-6">
      <div class="font-medium mb-2">{{ t('settings.asr.provider.title') }}</div>
      <select
        v-model="localSettings.active_provider"
        class="w-full px-3 py-2 border rounded"
      >
        <option
          v-for="p in asrStore.providers"
          :key="p.id"
          :value="p.id"
        >
          {{ t(`settings.asr.provider.options.${p.id}`) }}
        </option>
      </select>

      <div v-if="activeProviderInfo" class="mt-3 space-y-2">
        <div v-for="field in activeProviderInfo.config_fields" :key="field.key">
          <label class="block text-sm mb-1">
            {{ field.label }}
            <span v-if="field.required" class="text-red-500">*</span>
          </label>
          <!--
            field.key 是后端动态返回的字符串键（如 'api_key' / 'endpoint'），
            ProviderConfig 类型只声明了部分键，因此通过 unknown 双步转换为 Record<string, string>
            再索引（v-model 需要可写）。
          -->
          <input
            v-if="field.kind.name === 'secret'"
            type="password"
            v-model="providerCfgRecord[field.key]"
            class="w-full px-3 py-2 border rounded"
          />
          <input
            v-else
            type="text"
            v-model="providerCfgRecord[field.key]"
            :placeholder="field.kind.placeholder"
            class="w-full px-3 py-2 border rounded"
          />
        </div>
        <button
          class="mt-2 px-3 py-1 text-sm bg-blue-500 text-white rounded hover:bg-blue-600"
          @click="testConnection"
        >
          {{ t('settings.asr.provider.test') }}
        </button>
        <p
          v-if="lastTestResult"
          class="text-sm mt-1"
          :class="lastTestResult.ok ? 'text-green-600' : 'text-red-600'"
        >
          {{ lastTestResult.text }}
        </p>
      </div>
    </section>

    <section class="text-sm text-gray-500 space-y-1 border-t pt-4">
      <div>
        {{ t('settings.asr.status.mic') }}:
        {{ micStateText }}
      </div>
      <div>
        {{ t('settings.asr.status.vadLoaded') }}:
        {{ vadStateText }}
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { useAsrStore } from '@/stores/modules/settings/asr'
import { recordKeyUntilEscape } from '@/composables/useGlobalHotkey'
import { asrTestProvider } from '@/api/services/asr'
import type { AsrSettings, SendMode, ProviderInfo } from '@/api/services/asr'

const { t } = useI18n()
const asrStore = useAsrStore()

// 用 structuredClone 深拷贝，避免对 store 原对象产生副作用
const localSettings = ref<AsrSettings>(structuredClone(asrStore.settings))
const lastTestResult = ref<{ ok: boolean; text: string } | null>(null)

let saveTimer: number | null = null

const sendModeOptions = computed<{ value: SendMode; label: string }[]>(() => [
  { value: 'fill_only', label: t('settings.asr.sendMode.fillOnly') },
  { value: 'auto_send', label: t('settings.asr.sendMode.autoSend') },
  { value: 'queue', label: t('settings.asr.sendMode.queue') },
])

const activeProviderInfo = computed<ProviderInfo | undefined>(() =>
  asrStore.providers.find((p) => p.id === localSettings.value.active_provider),
)

const providerCfg = computed(() => {
  const id = localSettings.value.active_provider
  if (!localSettings.value.provider_configs[id]) {
    localSettings.value.provider_configs[id] = { api_key: '', endpoint: '' }
  }
  return localSettings.value.provider_configs[id]
})

// ProviderConfig 是后端约定的具名键（api_key / endpoint / extra），
// 而 config_field.key 是动态字符串，需要做 Record 桥接才能用 v-model 写入任意键。
const providerCfgRecord = computed(() => providerCfg.value as unknown as Record<string, string>)

const statusText = computed(() =>
  asrStore.lastError ? t('settings.asr.status.notReady') : t('settings.asr.status.ready'),
)
const statusClass = computed(() => (asrStore.lastError ? 'text-red-500' : 'text-green-500'))

const micStateText = computed(() => {
  switch (asrStore.micState) {
    case 'recording':
      return t('settings.asr.status.micActive')
    case 'denied':
      return t('settings.asr.status.micDenied')
    default:
      return t('settings.asr.status.micIdle')
  }
})

const vadStateText = computed(() =>
  asrStore.vadLoaded ? t('settings.asr.status.vadLoadedOk') : t('settings.asr.status.vadLoadedNo'),
)

onMounted(async () => {
  await asrStore.load()
  // 用 spread 完成顶层浅拷贝（settings 结构本身简单可序列化）；provider_configs 内部由
  // providerCfg 计算属性的懒初始化处理。spread 也足以让 v-model 写入不影响 store。
  localSettings.value = { ...asrStore.settings }
})

watch(
  localSettings,
  (s) => {
    if (saveTimer !== null) clearTimeout(saveTimer)
    saveTimer = window.setTimeout(() => {
      void asrStore.save(s)
    }, 500)
  },
  { deep: true },
)

async function recordHotkey() {
  localSettings.value.hotkey_combination = await recordKeyUntilEscape()
}

async function testConnection() {
  try {
    await asrTestProvider(localSettings.value.active_provider)
    lastTestResult.value = {
      ok: true,
      text: t('settings.asr.provider.testSuccess'),
    }
  } catch (e: unknown) {
    lastTestResult.value = {
      ok: false,
      text: t('settings.asr.provider.testFailed', { err: String(e) }),
    }
  }
}
</script>

<template>
  <div class="p-6 overflow-y-auto h-full">
    <!-- 标题栏（与 SettingsAdvanceOther 一致：brand 色 + 下边框分隔） -->
    <header class="pb-4 mb-6 border-b border-brand flex items-center justify-between">
      <h2 class="text-2xl text-brand font-semibold">{{ t('settings.asr.title') }}</h2>
      <span class="text-sm" :class="statusClass">{{ statusText }}</span>
    </header>

    <!-- 语音输入总开关（控制所有输入来源） -->
    <section class="mb-6">
      <Toggle
        :checked="localSettings.voice_input_enabled"
        @change="(v: boolean) => (localSettings.voice_input_enabled = v)"
      >
        <span class="font-medium">{{ t('settings.asr.voiceInput') }}</span>
        <span class="block text-sm text-gray-300 mt-0.5">{{ t('settings.asr.voiceInputHint') }}</span>
      </Toggle>
    </section>

    <!-- 自动语音识别开关 -->
    <section class="mb-6">
      <Toggle :checked="localSettings.auto_listen" @change="(v: boolean) => (localSettings.auto_listen = v)">
        <span class="font-medium">{{ t('settings.asr.autoListen') }}</span>
        <span class="block text-sm text-gray-300 mt-0.5">{{ t('settings.asr.autoListenHint') }}</span>
      </Toggle>
    </section>

    <!-- 快捷键开关 + 录制 -->
    <section class="mb-6">
      <Toggle
        :checked="localSettings.hotkey_enabled"
        @change="(v: boolean) => (localSettings.hotkey_enabled = v)"
      >
        <span class="font-medium">{{ t('settings.asr.hotkey.enable') }}</span>
      </Toggle>
      <div v-if="localSettings.hotkey_enabled" class="mt-3 flex items-center gap-3 pl-1">
        <span class="text-sm text-gray-300">
          {{ t('settings.asr.hotkey.combination') }}:
          <kbd
            class="px-2.5 py-1 rounded-md text-xs font-mono border border-white/20 bg-white/10"
          >{{ localSettings.hotkey_combination }}</kbd>
        </span>
        <button
          type="button"
          class="px-4 py-2 bg-brand text-white rounded-lg hover:bg-[#0056b3] transition-colors duration-200 text-sm"
          @click="recordHotkey"
        >
          {{ t('settings.asr.hotkey.record') }}
        </button>
      </div>
    </section>

    <!-- 识别完成后处理方式 -->
    <section class="mb-6">
      <div class="font-medium text-brand mb-3">{{ t('settings.asr.sendMode.title') }}</div>
      <div class="space-y-2">
        <label
          v-for="opt in sendModeOptions"
          :key="opt.value"
          class="flex items-center gap-2 cursor-pointer text-sm"
        >
          <input
            type="radio"
            :value="opt.value"
            v-model="localSettings.send_mode"
            class="accent-(--accent-color) w-4 h-4"
          />
          <span>{{ opt.label }}</span>
        </label>
      </div>
    </section>

    <!-- 识别服务商 -->
    <section class="mb-6">
      <div class="font-medium text-brand mb-3">{{ t('settings.asr.provider.title') }}</div>
      <select
        v-model="localSettings.active_provider"
        class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
      >
        <option
          v-for="p in asrStore.providers"
          :key="p.id"
          :value="p.id"
          class="text-black"
        >
          {{ t(`settings.asr.provider.options.${p.id}`) }}
        </option>
      </select>

      <div v-if="activeProviderInfo" class="mt-4 space-y-3">
        <!-- 模型自选下拉（仅提供模型清单的 provider 显示） -->
        <div v-if="asrModels.length > 0">
          <label class="block text-sm mb-1.5 font-medium">
            {{ t('settings.asr.model.label') }}
          </label>
          <select
            v-model="providerCfgRecord.model"
            class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
          >
            <option
              v-for="m in asrModels"
              :key="m.id"
              :value="m.id"
              class="text-black"
            >
              {{ m.display_name }}{{ m.is_default ? `（${t('settings.asr.model.default')}）` : '' }}
            </option>
          </select>
        </div>
        <div v-for="field in activeProviderInfo.config_fields" :key="field.key">
          <label class="block text-sm mb-1.5 font-medium">
            {{ field.label }}
            <span v-if="field.required" class="text-red-500">*</span>
          </label>
          <!--
            field.key 是后端动态返回的字符串键（如 'api_key' / 'endpoint'），
            ProviderConfig 类型只声明了部分键，因此通过 unknown 双步转换为 Record<string, string>
            再索引（v-model 需要可写）。
            field.kind 与后端 ConfigFieldKind 对齐：text / password / number / boolean。
          -->
          <input
            v-if="field.kind === 'password'"
            type="password"
            v-model="providerCfgRecord[field.key]"
            class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
          />
          <input
            v-else-if="field.kind === 'number'"
            type="number"
            v-model="providerCfgRecord[field.key]"
            :placeholder="field.placeholder"
            class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
          />
          <label
            v-else-if="field.kind === 'boolean'"
            class="flex items-center gap-2 cursor-pointer"
          >
            <input
              type="checkbox"
              v-model="providerCfgRecord[field.key]"
              class="accent-(--accent-color) w-4 h-4"
            />
          </label>
          <input
            v-else
            type="text"
            v-model="providerCfgRecord[field.key]"
            :placeholder="field.placeholder"
            class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
          />
        </div>
        <div class="flex items-center gap-3">
          <button
            type="button"
            class="px-4 py-2 bg-brand text-white rounded-lg hover:bg-[#0056b3] transition-colors duration-200 text-sm"
            @click="testConnection"
          >
            {{ testRecording ? t('settings.asr.provider.testingStop') : t('settings.asr.provider.test') }}
          </button>
          <p
            v-if="lastTestResult"
            class="text-sm max-w-md"
            :class="lastTestResult.ok ? 'text-green-400' : 'text-red-400'"
          >
            {{ lastTestResult.text }}
          </p>
        </div>
      </div>

      <!-- 流式识别开关：选中模型支持流式才可用 -->
      <div class="mt-4 pt-4 border-t border-white/10">
        <Toggle
          :checked="localSettings.stream_enabled"
          :disabled="!providerSupportsStreaming"
          @change="(v: boolean) => (localSettings.stream_enabled = v)"
        >
          <span class="font-medium">{{ t('settings.asr.streamMode') }}</span>
          <span class="block text-sm text-gray-300 mt-0.5">
            {{
              providerSupportsStreaming
                ? t('settings.asr.streamModeHint')
                : t('settings.asr.streamNotSupported')
            }}
          </span>
        </Toggle>
      </div>
    </section>

    <!-- 状态面板 -->
    <section class="text-sm text-gray-300 space-y-1.5 border-t border-white/10 pt-4">
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

import { Toggle } from '../../base'
import { useAsrStore } from '@/stores/modules/settings/asr'
import { recordKeyUntilEscape } from '@/composables/useGlobalHotkey'
import { asrListModels, asrRecognizeWav } from '@/api/services/asr'
import { pcmToWavPcm16, trimSilencePcm } from '@/utils/asrAudio'
import type { AsrSettings, SendMode, ProviderInfo } from '@/api/services/asr'

const { t, te } = useI18n()
const asrStore = useAsrStore()

// 深拷贝表单副本（不能用 structuredClone —— Pinia reactive Proxy 会抛
// DataCloneError，导致 setup 崩溃整页空白；JSON 序列化无此问题）
const localSettings = ref<AsrSettings>(JSON.parse(JSON.stringify(asrStore.settings)))
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

// ── 模型自选（仅 qwen 有模型清单；provider 切换时重新拉取） ──
const asrModels = computed(() => asrStore.models)
/** 当前生效模型：配置非空取配置，否则默认模型 */
const activeModel = computed(() => {
  const id = localSettings.value.provider_configs[localSettings.value.active_provider]?.model ?? ''
  return (
    asrStore.models.find((m) => m.id === id) ??
    asrStore.models.find((m) => m.is_default)
  )
})
watch(
  () => localSettings.value.active_provider,
  (id) => {
    ensureProviderConfig(id)
    void asrListModels(id)
      .then((list) => (asrStore.models = list))
      .catch(() => (asrStore.models = []))
  },
  { immediate: true },
)

// 流式开关可用性：当前生效模型的流式能力（模型级权威判定）
const providerSupportsStreaming = computed(
  () => activeModel.value?.supports_streaming ?? false,
)

// 切到不支持流式的模型 → 自动关闭流式开关（避免录音时后端报错）
watch(activeModel, (m) => {
  if (!m?.supports_streaming && localSettings.value.stream_enabled) {
    localSettings.value.stream_enabled = false
  }
})

// 流式开关 ↔ 模型自动同步：打开流式 → 切到流式模型；关闭 → 切到非流式模型。
// 模型与协议强绑定（流式模型只能走 WebSocket 端点，反之亦然），
// 设置层保持一致，后端回退兜底。
watch(
  () => localSettings.value.stream_enabled,
  (on) => {
    const m = activeModel.value
    if (!m) return
    const cfg = localSettings.value.provider_configs[localSettings.value.active_provider]
    if (on && !m.supports_streaming) {
      const sm = asrStore.models.find((x) => x.supports_streaming)
      if (sm && cfg) cfg.model = sm.id
    } else if (!on && m.supports_streaming) {
      const nm = asrStore.models.find((x) => !x.supports_streaming)
      if (nm && cfg) cfg.model = nm.id
    }
  },
)

// provider 切换 / 挂载时显式初始化缺失配置（不在渲染期突变 state）
function ensureProviderConfig(id: string) {
  if (!localSettings.value.provider_configs[id]) {
    localSettings.value.provider_configs[id] = { api_key: '', endpoint: '', model: '', extra: {} }
  }
}
watch(
  () => localSettings.value.active_provider,
  (id) => ensureProviderConfig(id),
  { immediate: true },
)

// ProviderConfig 是后端约定的具名键（api_key / endpoint / extra），
// 而 config_field.key 是动态字符串，需要做 Record 桥接才能用 v-model 写入任意键。
// 只读：写路径走 watch 的 ensureProviderConfig + debounce save。
const providerCfg = computed(() => {
  const id = localSettings.value.active_provider
  return localSettings.value.provider_configs[id] ?? { api_key: '', endpoint: '' }
})
const providerCfgRecord = computed(() => providerCfg.value as unknown as Record<string, string>)

const statusText = computed(() =>
  asrStore.lastError ? t('settings.asr.status.notReady') : t('settings.asr.status.ready'),
)
const statusClass = computed(() => (asrStore.lastError ? 'text-red-400' : 'text-green-400'))

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
      void asrStore.save(s).catch((e) => console.warn('[ASR] autosave failed:', e))
    }, 500)
  },
  { deep: true },
)

async function recordHotkey() {
  localSettings.value.hotkey_combination = await recordKeyUntilEscape()
}

// ── 测试连接：完整识别链路（录音 4 秒 → 16k PCM → recognize → 显示文本） ──
// 不用 MediaRecorder（webm 在 WebView2 decodeAudioData 会失败），
// 与 useAsrInput 同路径：ScriptProcessor 直接采 16k f32 PCM → pcmToWavPcm16。
const testRecording = ref(false)
let testStream: MediaStream | null = null
let testCtx: AudioContext | null = null
let testProcessor: ScriptProcessorNode | null = null
let testPcm: number[] = []
let testTimer: number | null = null

async function testConnection() {
  try {
    // 先保存表单值：确保后端 registry 用的是用户刚填的 api_key（消除 500ms debounce 竞态）
    await asrStore.save(localSettings.value)

    if (!testRecording.value) {
      // 第一阶段：开始录音（4 秒后自动停止）
      testStream = await navigator.mediaDevices.getUserMedia({
        audio: {
          sampleRate: 16000,
          channelCount: 1,
          echoCancellation: true,
          noiseSuppression: true,
        },
      })
      testCtx = new AudioContext({ sampleRate: 16000 })
      const src = testCtx.createMediaStreamSource(testStream)
      testProcessor = testCtx.createScriptProcessor(1024, 1, 1)
      src.connect(testProcessor)
      // 输出接零增益节点而非 destination，避免把采集流回放
      const silence = testCtx.createGain()
      silence.gain.value = 0
      testProcessor.connect(silence)
      silence.connect(testCtx.destination)
      testPcm = []
      testProcessor.onaudioprocess = (e) => {
        testPcm.push(...e.inputBuffer.getChannelData(0))
      }
      testRecording.value = true
      lastTestResult.value = { ok: true, text: t('settings.asr.provider.testing') }
      testTimer = window.setTimeout(() => void finishTestRecording(), 4000)
      return
    }

    // 第二阶段：手动停止（点按钮提前结束）
    await finishTestRecording()
  } catch (e: unknown) {
    // 录音初始化失败（权限等）或识别失败
    const raw = String(e)
    const [code, ...rest] = raw.split('|')
    const key = `settings.asr.errors.${code}`
    let text = te(key) ? t(key) : raw
    if (rest.length > 0) {
      text += `（${rest.join('|')}）`
    }
    lastTestResult.value = { ok: false, text }
    cleanupTestRecording()
  }
}

/** 停止录音 → PCM 合成 WAV → 走完整识别链路 → 显示识别文本 */
async function finishTestRecording() {
  if (testTimer !== null) {
    clearTimeout(testTimer)
    testTimer = null
  }
  const pcm = testPcm
  cleanupTestRecording()
  try {
    // 裁剪首尾静音，只送语音段
    const wav = pcmToWavPcm16(trimSilencePcm(pcm))
    if (wav.byteLength <= 44) {
      lastTestResult.value = { ok: false, text: t('settings.asr.provider.testNoSpeech') }
      return
    }
    const result = await asrRecognizeWav({
      providerId: localSettings.value.active_provider,
      wavBytes: Array.from(wav),
      languageHint: null,
    })
    lastTestResult.value = {
      ok: true,
      text: t('settings.asr.provider.testResult', {
        text: result.text || t('settings.asr.provider.testNoSpeech'),
      }),
    }
  } catch (e: unknown) {
    const raw = String(e)
    const [code, ...rest] = raw.split('|')
    const key = `settings.asr.errors.${code}`
    let text = te(key) ? t(key) : raw
    if (rest.length > 0) {
      text += `（${rest.join('|')}）`
    }
    lastTestResult.value = { ok: false, text }
  }
}

function cleanupTestRecording() {
  if (testTimer !== null) {
    clearTimeout(testTimer)
    testTimer = null
  }
  try {
    testProcessor?.disconnect()
  } catch {
    /* ignore */
  }
  testProcessor = null
  void testCtx?.close().catch(() => {})
  testCtx = null
  testStream?.getTracks().forEach((t) => t.stop())
  testStream = null
  testPcm = []
  testRecording.value = false
}
</script>

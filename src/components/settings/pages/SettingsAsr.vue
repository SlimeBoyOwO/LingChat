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

    <!-- VAD 静音计时（自动模式：停止说话后等多久才结束录音） -->
    <section class="mb-6">
      <label class="block text-sm mb-1.5 font-medium">{{ t('settings.asr.vadSilence') }}</label>
      <input
        type="number"
        min="100"
        max="3000"
        step="100"
        v-model.number="localSettings.vad_silence_ms"
        class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
      />
      <p class="block text-sm text-gray-300 mt-1.5">{{ t('settings.asr.vadSilenceHint') }}</p>
    </section>

    <!-- 能量监测缓冲期（自动模式：TTS 播完恢复监听后多久内不触发录音） -->
    <section class="mb-6">
      <label class="block text-sm mb-1.5 font-medium">{{ t('settings.asr.energyWarmup') }}</label>
      <input
        type="number"
        min="0"
        max="2000"
        step="100"
        v-model.number="localSettings.energy_warmup_ms"
        class="w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
      />
      <p class="block text-sm text-gray-300 mt-1.5">{{ t('settings.asr.energyWarmupHint') }}</p>
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

      <!-- 服务商选择：provider 由后端 list_provider_info 动态驱动 -->
      <label class="block text-sm mb-1.5 font-medium">{{ t('settings.asr.provider.providerSelect') }}</label>
      <select
        v-model="localSettings.active_provider"
        class="w-full px-3 py-2.5 border rounded-lg text-sm text-sky-400 bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200 mb-4"
      >
        <option v-for="p in asrStore.providers" :key="p.id" :value="p.id">
          {{ p.display_name }}{{ p.description ? `（${p.description}）` : '' }}
        </option>
      </select>

        <div v-if="activeProviderInfo" class="space-y-3">
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
          <!-- 模型下拉：provider 有动态模型清单（llama-asr 从服务端 /v1/models 拉取）
               时优先下拉选择；拉取失败/无清单回退文本输入 -->
          <div v-if="field.key === 'model' && asrStore.models.length > 0" class="flex gap-2">
            <select
              v-model="providerCfgRecord[field.key]"
              class="flex-1 px-3 py-2.5 border rounded-lg text-sm text-sky-400 bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
            >
              <option v-for="m in asrStore.models" :key="m.id" :value="m.id">{{ m.display_name }}</option>
            </select>
            <button
              type="button"
              :title="t('settings.asr.provider.modelRefresh')"
              class="px-3 rounded-lg border border-white/15 bg-white/5 text-white/60 hover:text-white/80 hover:bg-white/10 transition-colors"
              @click="refreshModels"
            >
              ↻
            </button>
          </div>
          <input
            v-else-if="field.kind === 'password'"
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
          <p
            v-if="field.key === 'model' && modelListError"
            class="text-red-400 text-sm mt-1"
          >
            {{ modelListError }}
          </p>
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
              !providerSupportsStreaming
                ? t('settings.asr.streamNotSupported')
                : localSettings.active_provider === 'llama-asr'
                  ? t('settings.asr.streamModeHintLocal')
                  : t('settings.asr.streamModeHint')
            }}
          </span>
        </Toggle>
      </div>
    </section>

    <!-- 状态面板：只保留 VAD 模型状态（init_asr 失败诊断的关键信号，
         麦克风状态在设置页恒为空闲无信息量，已移除） -->
    <section class="text-sm text-gray-300 border-t border-white/10 pt-4">
      <div>
        {{ t('settings.asr.status.vadLoaded') }}:
        <span :class="vadStateClass">{{ vadStateText }}</span>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { Toggle } from '../../base'
import { useAsrStore } from '@/stores/modules/settings/asr'
import { asrListModels, asrRecognizeWav, asrGetStatus } from '@/api/services/asr'
import { pcmToWavPcm16, trimSilencePcm } from '@/utils/asrAudio'
import { parseAsrError } from '@/utils/asrError'
import type { AsrSettings, SendMode, ProviderInfo } from '@/api/services/asr'

const { t, te } = useI18n()
const asrStore = useAsrStore()

// 深拷贝表单副本（不能用 structuredClone —— Pinia reactive Proxy 会抛
// DataCloneError，导致 setup 崩溃整页空白；JSON 序列化无此问题）
const localSettings = ref<AsrSettings>(JSON.parse(JSON.stringify(asrStore.settings)))
const lastTestResult = ref<{ ok: boolean; text: string } | null>(null)

let saveTimer: number | null = null
/** 初始化完成标记：onMounted 赋值后置 true，跳过一次初始化触发的保存 */
let initialized = false

const sendModeOptions = computed<{ value: SendMode; label: string }[]>(() => [
  { value: 'fill_only', label: t('settings.asr.sendMode.fillOnly') },
  { value: 'auto_send', label: t('settings.asr.sendMode.autoSend') },
])

// ── 模型预设：来自后端模型元数据（ModelInfo.endpoint，单一数据源）──
// 选中模型时同步填入 model + 端点预设（用户手改的 endpoint 被覆盖，与 LLM 预设一致）；
// llama-asr 的端点与模型无关（endpoint=None），只填 model。

/** 应用模型预设：填 model + 端点预设（来自后端模型元数据） */
function applyAsrPreset(model: string) {
  const cfg = localSettings.value.provider_configs[localSettings.value.active_provider]
  const m = asrStore.models.find((x) => x.id === model)
  if (!cfg || !m) return
  cfg.model = m.id
  if (m.endpoint) {
    cfg.endpoint = m.endpoint
  }
}

const activeProviderInfo = computed<ProviderInfo | undefined>(() =>
  asrStore.providers.find((p) => p.id === localSettings.value.active_provider),
)

/** 模型列表拉取失败信息（llama-asr 服务未启动等），显示在模型字段下方 */
const modelListError = ref('')

/** 拉取当前 provider 的模型清单；失败时清空列表并记录错误（llama-asr 回退文本输入） */
async function loadModels(id: string) {
  try {
    asrStore.models = await asrListModels(id)
    modelListError.value = ''
  } catch (e) {
    asrStore.models = []
    const info = parseAsrError(e)
    modelListError.value = t('settings.asr.provider.modelListFailed', {
      err: info.detail ?? info.code,
    })
  }
}

/** 手动刷新模型列表（llama-server 换模型/重启后重新拉取） */
function refreshModels() {
  void loadModels(localSettings.value.active_provider)
}

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
    void loadModels(id)
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
// 设置层保持一致，后端回退兜底。切模型用 applyAsrPreset（endpoint 同步填入）。
watch(
  () => localSettings.value.stream_enabled,
  (on) => {
    const m = activeModel.value
    if (!m) return
    if (on && !m.supports_streaming) {
      const sm = asrStore.models.find((x) => x.supports_streaming)
      if (sm) applyAsrPreset(sm.id)
    } else if (!on && m.supports_streaming) {
      const nm = asrStore.models.find((x) => !x.supports_streaming)
      if (nm) applyAsrPreset(nm.id)
    }
  },
)

// provider 切换 / 挂载时显式初始化缺失配置（不在渲染期突变 state）
function ensureProviderConfig(id: string) {
  const cfg =
    localSettings.value.provider_configs[id] ?? { api_key: '', endpoint: '', model: '', extra: {} }
  localSettings.value.provider_configs[id] = cfg
  // 后端 default_value 兜底空字段（如 llama-asr 的 endpoint/model 默认值）：
  // 用户不填也能开箱即用；已有值不覆盖（输入框渲染只用了 placeholder，
  // 从未落过 default_value，这里补上——qwen 的默认 endpoint 同理受益）
  const record = cfg as unknown as Record<string, string>
  const info = asrStore.providers.find((p) => p.id === id)
  info?.config_fields.forEach((f) => {
    if (f.default_value && !record[f.key]) {
      record[f.key] = f.default_value
    }
  })
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

const statusText = computed(() => {
  if (!asrStore.lastError) return t('settings.asr.status.ready')
  // 有错误：显示错误摘要（i18n 文案 + 原始 code 兜底）
  const errKey = `settings.asr.errors.${asrStore.lastError}`
  const errText = te(errKey) ? t(errKey) : asrStore.lastError
  return `${t('settings.asr.status.notReady')}（${errText}）`
})
const statusClass = computed(() => (asrStore.lastError ? 'text-red-400' : 'text-green-400'))

const vadStateText = computed(() =>
  asrStore.vadLoaded ? t('settings.asr.status.vadLoadedOk') : t('settings.asr.status.vadLoadedNo'),
)
const vadStateClass = computed(() => (asrStore.vadLoaded ? 'text-green-400' : 'text-red-400'))

onMounted(async () => {
  await asrStore.load()
  // 用 spread 完成顶层浅拷贝（settings 结构本身简单可序列化）；provider_configs 内部由
  // providerCfg 计算属性的懒初始化处理。spread 也足以让 v-model 写入不影响 store。
  localSettings.value = { ...asrStore.settings }
  // 初始化赋值不算"用户更改"：赋值在前、置位在后，sync watch 回调在赋值瞬间
  // 同步执行时 initialized 仍为 false 而被跳过——避免每次打开设置页都触发一次
  // 无意义的 asr_set_settings（后端重应用静音计时 + 重建 provider 并刷日志）
  initialized = true
  // 查询式获取 VAD 状态：asr://vad_ready 事件在启动早期发射，前端监听器注册
  // 晚于事件会丢失（Tauri 事件不缓存历史）——以查询结果为准，无竞态
  asrGetStatus()
    .then((s) => asrStore.setVadLoaded(s.vad_loaded))
    .catch((e) => console.warn('[ASR] 查询状态失败:', e))
})

watch(
  localSettings,
  (s) => {
    // 初始化赋值（onMounted 的 spread 拷贝）不触发保存——打开设置页不改任何值
    // 不应该向后端重写设置（asr_set_settings 会重应用 VAD 计时并重建 provider）。
    // flush:'sync' 保证回调同步执行：onMounted 赋值时 initialized 仍是 false 被跳过，
    // 置位后用户的实际修改才走保存。
    if (!initialized) return
    if (saveTimer !== null) clearTimeout(saveTimer)
    saveTimer = window.setTimeout(() => {
      void asrStore.save(s).catch((e) => console.warn('[ASR] autosave failed:', e))
    }, 500)
  },
  { deep: true, flush: 'sync' },
)

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
    const info = parseAsrError(e)
    const key = `settings.asr.errors.${info.code}`
    let text = te(key) ? t(key) : (info.code || String(e))
    if (info.detail) {
      text += `（${info.detail}）`
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
    const info = parseAsrError(e)
    const key = `settings.asr.errors.${info.code}`
    let text = te(key) ? t(key) : (info.code || String(e))
    if (info.detail) {
      text += `（${info.detail}）`
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

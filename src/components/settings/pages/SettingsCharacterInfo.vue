<template>
  <Transition name="modal">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-sm"
      @click="handleClose"
    >
      <div
        class="bg-[linear-gradient(135deg,rgba(255,255,255,0.15)_0%,rgba(255,255,255,0.05)_100%)] backdrop-blur-[30px] backdrop-saturate-180 rounded-3xl shadow-[0_20px_60px_rgba(0,0,0,0.4),inset_0_0_1px_rgba(255,255,255,0.3)] border border-white/20 w-full max-w-4xl h-[85vh] flex flex-col overflow-hidden text-white"
        @click.stop
      >
        <!-- Header -->
        <div
          class="flex items-center justify-between p-6 border-b border-white/10 bg-[linear-gradient(180deg,rgba(255,255,255,0.1)_0%,rgba(255,255,255,0.05)_100%)]"
        >
          <div class="flex items-center gap-4">
            <div
              class="w-12 h-12 rounded-xl bg-white/10 flex items-center justify-center shadow-inner"
            >
              <Icon icon="setting" />
            </div>
            <div>
              <h2 class="text-xl font-bold m-0 drop-shadow-[0_2px_4px_rgba(0,0,0,0.3)]">
                {{ title }} - 配置编辑
              </h2>
              <p class="text-sm text-white/50 m-0">修改角色的详细设置</p>
            </div>
          </div>
          <button
            class="w-9 h-9 rounded-full border-none bg-white/10 text-white flex items-center justify-center cursor-pointer transition-all duration-200 hover:bg-white/20 hover:rotate-90"
            @click="handleClose"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="24"
              height="24"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>

        <!-- Content -->
        <div class="flex-1 overflow-hidden flex flex-row">
          <!-- Sidebar (vertical scrollable for narrow viewports) -->
          <div class="w-44 shrink-0 bg-black/10 flex flex-col gap-2 p-3 border-r border-white/10 overflow-y-auto tab-sidebar-scroll">
            <button
              v-for="tab in tabs"
              :key="tab.id"
              class="w-full text-left px-4 py-2.5 rounded-xl border-none bg-transparent text-white/60 cursor-pointer transition-all duration-200 font-medium hover:bg-white/5 hover:text-white"
              :class="{
                'bg-[rgba(94,114,228,0.2)] text-[#79d9ff]! font-semibold!': activeTab === tab.id,
              }"
              @click="activeTab = tab.id"
            >
              {{ tab.label }}
            </button>
          </div>

          <!-- Tab Panels -->
          <div class="flex-1 overflow-y-auto p-6 relative">
            <div v-if="loading" class="flex items-center justify-center h-full">
              <div
                class="w-10 h-10 border-3 border-white/10 border-t-[#5e72e4] rounded-full animate-spin"
              ></div>
            </div>

            <div v-else class="max-w-3xl mx-auto space-y-6">
              <!-- Data-Driven Form (tabs with schemas) -->
              <div v-if="currentTabConfig" class="space-y-4">
                <div
                  v-for="(field, index) in currentTabFields"
                  :key="index"
                  class="flex flex-col gap-2"
                >
                  <template v-if="!field.visibleIf || field.visibleIf(localSettings)">
                    <label :for="field.key" class="text-[13px] text-white/60 font-medium"
                      >{{ field.label }} ({{ field.key }})</label
                    >
                    <input
                      v-if="field.type === 'text' || field.type === 'number'"
                      :id="field.key"
                      v-model="fieldModel(field).value"
                      :type="field.type"
                      :step="field.step"
                      :placeholder="field.placeholder"
                      class="form-control bg-black/20 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm outline-none transition-all duration-200"
                      @change="handleFieldChange(field)"
                    />
                    <textarea
                      v-else-if="field.type === 'textarea'"
                      :id="field.key"
                      v-model="fieldModel(field).value"
                      :rows="field.rows || 4"
                      class="form-control bg-black/20 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm outline-none transition-all duration-200 font-mono leading-relaxed"
                    ></textarea>
                    <select
                      v-else-if="field.type === 'select'"
                      :id="field.key"
                      v-model="fieldModel(field).value"
                      class="form-control bg-black/20 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm outline-none transition-all duration-200"
                      @change="handleFieldChange(field)"
                    >
                      <option
                        v-for="opt in resolveFieldOptions(field)"                        :key="opt.value"
                        :value="opt.value"
                        class="bg-[#333] text-white"
                      >
                        {{ opt.label }}
                      </option>
                    </select>
                  </template>
                </div>
              </div>

              <!-- Clothes Tab (custom UI, outside data-driven block) -->
              <div v-if="activeTab === 'clothes'" class="space-y-4">
                <div class="flex items-center justify-between">
                  <h3 class="text-sm font-bold text-white/70">服装列表</h3>
                  <button
                    class="px-3 py-1.5 rounded-lg border-none bg-[#5e72e4] text-white text-xs cursor-pointer hover:bg-[#4a5acf] transition-colors"
                    @click="addClothesItem"
                  >
                    + 添加服装
                  </button>
                </div>
                <div
                  v-for="(item, idx) in clothesList"
                  :key="idx"
                  class="p-4 bg-white/5 rounded-xl border border-white/10 space-y-3"
                >
                  <div class="flex items-center justify-between">
                    <span class="text-sm font-medium text-white/80">服装 #{{ idx + 1 }}</span>
                    <button
                      class="w-6 h-6 rounded-full border-none bg-red-500/20 text-red-400 cursor-pointer text-xs hover:bg-red-500/40 transition-colors"
                      @click="removeClothesItem(idx)"
                    >
                      x
                    </button>
                  </div>
                  <div class="flex flex-col gap-2">
                    <label class="text-[13px] text-white/60 font-medium">name</label>
                    <input
                      v-model="item.name"
                      type="text"
                      class="form-control bg-black/20 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm outline-none transition-all duration-200"
                    />
                  </div>
                  <div class="flex flex-col gap-2">
                    <label class="text-[13px] text-white/60 font-medium">prompt</label>
                    <textarea
                      v-model="item.prompt"
                      rows="3"
                      class="form-control bg-black/20 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm outline-none transition-all duration-200 font-mono leading-relaxed"
                    ></textarea>
                  </div>
                </div>
                <div v-if="clothesList.length === 0" class="text-sm text-white/40 text-center py-8">
                  暂无服装配置，点击"添加服装"创建
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Footer -->
        <div
          class="p-4 border-t border-white/10 flex justify-end gap-3 bg-[linear-gradient(180deg,rgba(255,255,255,0.05)_0%,rgba(255,255,255,0.1)_100%)]"
        >
          <button
            class="px-5 py-2 rounded-[20px] text-sm font-medium cursor-pointer transition-all duration-200 border-none bg-white/10 text-white hover:bg-white/20"
            @click="handleClose"
          >
            取消
          </button>
          <button
            class="px-5 py-2 rounded-[20px] text-sm font-medium cursor-pointer transition-all duration-200 border-none bg-[#5e72e4] text-white disabled:opacity-60 disabled:cursor-not-allowed hover:enabled:bg-[#4a5acf] hover:enabled:-translate-y-px hover:enabled:shadow-[0_4px_12px_rgba(94,114,228,0.3)]"
            :disabled="saving"
            @click="saveSettings"
          >
            <span
              v-if="saving"
              class="inline-block w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin mr-2"
            ></span>
            {{ saving ? '保存中...' : '保存更改' }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { getRoleSettings, updateRoleSettings } from '../../../api/services/character'
import { Icon } from '../../base'
import { useDialogStore } from '../../../stores/modules/ui/dialog'
import * as TtsLocal from '../../../api/services/tts/tts-local'

const props = defineProps<{
  visible: boolean
  roleId: number | null
  title?: string
}>()

const emit = defineEmits(['close', 'saved'])

const activeTab = ref('basic')
const loading = ref(false)
const saving = ref(false)
const dialogStore = useDialogStore()
const localSettings = ref<any>({})
const installedVoices = ref<TtsLocal.VoiceRecord[]>([])

async function refreshLocalVoices(): Promise<void> {
  try {
    const snapshot = await TtsLocal.listInstalled()
    installedVoices.value = snapshot.voices
  } catch (error) {
    console.warn('refreshLocalVoices failed', error)
    installedVoices.value = []
  }
}

const tabs = [
  { id: 'basic', label: '基本信息' },
  { id: 'prompts', label: '提示词' },
  { id: 'visuals', label: '视觉效果' },
  { id: 'clothes', label: '服装' },
  { id: 'pet', label: '桌宠' },
  { id: 'voice', label: '语音设置' },
]

const voiceModelKeys = [
  'sva_speaker_id',
  'sbv2_name',
  'sbv2_speaker_id',
  'bv2_speaker_id',
  'sbv2api_name',
  'sbv2api_speaker_id',
  'gsv_voice_text',
  'gsv_voice_filename',
  'gsv_gpt_model_name',
  'gsv_sovits_model_name',
  'aivis_model_uuid',
  'opentts_voice',
] as const

// --- Schema Definition ---

type FieldType = 'text' | 'number' | 'textarea' | 'select'

interface FieldOption {
  label: string
  value: string
  visibleIf?: (settings: any) => boolean
}

interface FieldSchema {
  key: string
  label: string
  type: FieldType
  rows?: number
  step?: string
  placeholder?: string
  options?: FieldOption[]
  // Dynamic options computed from refs/state. Overrides options when set.
  dynamicOptions?: () => { label: string; value: string }[]
  visibleIf?: (settings: any) => boolean
  isVoiceModel?: boolean
  realtime?: boolean
  // When set, the field reads/writes into localSettings.value[parent][key].
  // The parent object is auto-initialised to {} on first write if missing.
  parent?: string
}

const fieldOptions = (field: FieldSchema) => {
  return (field.options || []).filter(
    (option) => !option.visibleIf || option.visibleIf(localSettings.value),
  )
}

const schemas: Record<string, FieldSchema[]> = {
  basic: [
    { key: 'ai_name', label: 'AI 名称', type: 'text' },
    { key: 'ai_subtitle', label: 'AI 副标题', type: 'text' },
    { key: 'user_name', label: '用户名称', type: 'text' },
    { key: 'user_subtitle', label: '用户副标题', type: 'text' },
    { key: 'title', label: '角色标题', type: 'text' },
    { key: 'info', label: '角色介绍', type: 'textarea', rows: 4 },
  ],
  prompts: [
    { key: 'system_prompt', label: '系统提示词', type: 'textarea', rows: 10 },
    { key: 'system_prompt_example', label: '对话示例', type: 'textarea', rows: 6 },
    { key: 'system_prompt_example_old', label: '旧版兼容对话示例', type: 'textarea', rows: 4 },
  ],
  visuals: [
    { key: 'scale', label: '缩放', type: 'number', step: '0.01' },
    { key: 'offset_x', label: '水平偏移', type: 'number', step: '0.1' },
    { key: 'offset_y', label: '垂直偏移', type: 'number', step: '0.1' },
    { key: 'bubble_top', label: '气泡顶部距离', type: 'number' },
    { key: 'bubble_left', label: '气泡左侧距离', type: 'number' },
    { key: 'thinking_message', label: '思考消息文本', type: 'text' },
  ],
  pet: [
    { key: 'scale_p', label: '桌宠缩放', type: 'number', step: '0.01' },
    { key: 'offset_x_p', label: '桌宠水平偏移', type: 'number', step: '0.1' },
    { key: 'offset_y_p', label: '桌宠垂直偏移', type: 'number', step: '0.1' },
  ],
  voice: [
    {
      key: 'tts_type',
      label: 'TTS 类型',
      type: 'select',
      realtime: true,
      options: [
        { label: 'sva', value: 'sva' },
        { label: 'sbv2', value: 'sbv2' },
        { label: 'bv2', value: 'bv2' },
        { label: 'sbv2api', value: 'sbv2api' },
        { label: 'gsv', value: 'gsv' },
        { label: 'aivis', value: 'aivis' },
        { label: 'opentts', value: 'opentts' },
        { label: '本地 SBV2 API', value: 'localsbv2api' },
      ],
    },

    {
      key: 'voice_lang',
      label: '语音语言',
      type: 'select',
      realtime: true,
      options: [
        { label: '日语', value: 'ja' },
        { label: '中文', value: 'zh' },
        {
          label: '英语',
          value: 'en',
          visibleIf: (s) =>
            s.tts_type === 'gsv' || s.tts_type === 'opentts' || s.tts_type === 'sbv2',
        },
        {
          label: '韩语',
          value: 'ko',
          visibleIf: (s) => s.tts_type === 'gsv' || s.tts_type === 'opentts',
        },
      ],
    },

    {
      key: 'sva_speaker_id',
      label: 'sva_speaker_id',
      type: 'text',
      isVoiceModel: true,
      visibleIf: (s) => s.tts_type === 'sva',
    },

    {
      key: 'sbv2_name',
      label: 'sbv2_name',
      type: 'text',
      isVoiceModel: true,
      realtime: true,
      visibleIf: (s) => s.tts_type === 'sbv2',
    },
    {
      key: 'sbv2_speaker_id',
      label: 'sbv2_speaker_id',
      type: 'text',
      isVoiceModel: true,
      realtime: true,
      visibleIf: (s) => s.tts_type === 'sbv2',
    },

    {
      key: 'bv2_speaker_id',
      label: 'bv2_speaker_id',
      type: 'text',
      isVoiceModel: true,
      visibleIf: (s) => s.tts_type === 'bv2',
    },

    {
      key: 'sbv2api_name',
      label: 'sbv2api_name',
      type: 'text',
      isVoiceModel: true,
      realtime: true,
      visibleIf: (s) => s.tts_type === 'sbv2api',
    },
    {
      key: 'sbv2api_speaker_id',
      label: 'sbv2api_speaker_id',
      type: 'text',
      isVoiceModel: true,
      realtime: true,
      visibleIf: (s) => s.tts_type === 'sbv2api',
    },

    {
      key: 'gsv_voice_text',
      label: 'gsv_voice_text',
      type: 'text',
      isVoiceModel: true,
      realtime: true,
      visibleIf: (s) => s.tts_type === 'gsv',
    },
    {
      key: 'gsv_voice_filename',
      label: 'gsv_voice_filename',
      type: 'text',
      isVoiceModel: true,
      realtime: true,
      visibleIf: (s) => s.tts_type === 'gsv',
    },
    {
      key: 'gsv_gpt_model_name',
      label: 'gsv_gpt_model_name',
      type: 'text',
      isVoiceModel: true,
      realtime: true,
      visibleIf: (s) => s.tts_type === 'gsv',
    },
    {
      key: 'gsv_sovits_model_name',
      label: 'gsv_sovits_model_name',
      type: 'text',
      isVoiceModel: true,
      realtime: true,
      visibleIf: (s) => s.tts_type === 'gsv',
    },

    {
      key: 'aivis_model_uuid',
      label: 'aivis_model_uuid',
      type: 'text',
      isVoiceModel: true,
      visibleIf: (s) => s.tts_type === 'aivis',
    },

    // --- Local SBV2 (localsbv2api) ---
    {
      key: 'sbv2_local_voice_id',
      parent: 'voice_models',
      label: '本地语音 ID',
      type: 'select',
      dynamicOptions: () =>
        installedVoices.value.length === 0
          ? [{ label: '未安装本地模型（请先在 TTS 设置中导入）', value: '' }]
          : installedVoices.value.map((voice) => ({
              label: voice.display_name
                ? `${voice.display_name} (${voice.voice_id})`
                : voice.voice_id,
              value: voice.voice_id,
            })),
      visibleIf: (s) => s.tts_type === 'localsbv2api',
    },
    {
      key: 'sbv2_local_speaker_id',
      parent: 'voice_models',
      label: '说话人 ID',
      type: 'number',
      step: '1',
      visibleIf: (s) => s.tts_type === 'localsbv2api',
    },
    {
      key: 'sbv2_local_style_id',
      parent: 'voice_models',
      label: '风格 ID',
      type: 'number',
      step: '1',
      visibleIf: (s) => s.tts_type === 'localsbv2api',
    },
    {
      key: 'sbv2_local_length_scale',
      parent: 'voice_models',
      label: '长度缩放 (length_scale)',
      type: 'number',
      step: '0.05',
      visibleIf: (s) => s.tts_type === 'localsbv2api',
    },
    {
      key: 'sbv2_local_sdp_ratio',
      parent: 'voice_models',
      label: 'SDP 噪声比',
      type: 'number',
      step: '0.05',
      visibleIf: (s) => s.tts_type === 'localsbv2api',
    },
    {
      key: 'sbv2_local_cloud_fallback_model',
      parent: 'voice_models',
      label: '本地 TTS 云端备用模型',
      type: 'text',
      placeholder: '本地 TTS 关闭时使用，可留空',
      visibleIf: (s) => s.tts_type === 'localsbv2api',
    },
    {
      key: 'sbv2_local_cloud_fallback_speaker_id',
      parent: 'voice_models',
      label: '本地 TTS 云端备用说话人 ID',
      type: 'text',
      placeholder: '本地 TTS 关闭时使用，可留空',
      visibleIf: (s) => s.tts_type === 'localsbv2api',
    },
    {
      key: 'opentts_voice',
      label: 'OpenTTS 音色标识',
      type: 'text',
      isVoiceModel: true,
      realtime: true,
      placeholder: '留空则使用高级设置中的全局音色标识',
      visibleIf: (s) => s.tts_type === 'opentts',
    },
  ],
}

// --- Computed Properties ---

const currentTabConfig = computed(() => schemas[activeTab.value])

// voice model 子区域已移除，所有字段统一在主表单渲染
const currentTabFields = computed(() => {
  return currentTabConfig.value || []
})

const resolveFieldOptions = (field: FieldSchema) => {
  if (field.dynamicOptions) {
    return field.dynamicOptions()
  }
  return field.options ?? []
}

const ensureVoiceModels = () => {
  if (
    !localSettings.value.voice_models ||
    typeof localSettings.value.voice_models !== "object" ||
    Array.isArray(localSettings.value.voice_models)
  ) {
    localSettings.value.voice_models = {}
  }
  return localSettings.value.voice_models as Record<string, unknown>
}

const migrateLegacyVoiceModelFields = () => {
  const voiceModels = ensureVoiceModels()
  for (const key of voiceModelKeys) {
    const legacyValue = localSettings.value[key]
    if ((voiceModels[key] === undefined || voiceModels[key] === null) && legacyValue != null) {
      voiceModels[key] = legacyValue
    }
    delete localSettings.value[key]
  }
}

const fieldModel = (field: FieldSchema) => {
  return computed({
    get: () => {
      let target: any
      if (field.parent) {
        const parentObj = localSettings.value[field.parent]
        target = (parentObj && typeof parentObj === "object") ? parentObj : (localSettings.value[field.parent] = {})
      } else if (field.isVoiceModel) {
        target = ensureVoiceModels()
      } else {
        target = localSettings.value
      }
      return target[field.key]
    },
    set: (val: any) => {
      const coerced = field.type === "number" ? Number(val) : val
      let target: any
      if (field.parent) {
        if (!localSettings.value[field.parent] || typeof localSettings.value[field.parent] !== "object") {
          localSettings.value[field.parent] = {}
        }
        target = localSettings.value[field.parent]
      } else if (field.isVoiceModel) {
        target = ensureVoiceModels()
      } else {
        target = localSettings.value
      }
      target[field.key] = coerced
    },
  })
}

const clothesList = computed({
  get: () => {
    if (!Array.isArray(localSettings.value.clothes)) {
      localSettings.value.clothes = []
    }
    return localSettings.value.clothes as Array<{ name: string; prompt: string }>
  },
  set: (val) => {
    localSettings.value.clothes = val
  },
})

const addClothesItem = () => {
  if (!Array.isArray(localSettings.value.clothes)) {
    localSettings.value.clothes = []
  }
  localSettings.value.clothes.push({ name: '', prompt: '' })
}

const removeClothesItem = (idx: number) => {
  if (Array.isArray(localSettings.value.clothes)) {
    localSettings.value.clothes.splice(idx, 1)
  }
}

// --- Watchers & Methods ---

watch(
  () => props.visible,
  async (newVal) => {
    if (!newVal) {
      clearRealtimeSaveTimer()
    }
    if (newVal && props.roleId) {
      loading.value = true
      try {
        const data = await getRoleSettings(props.roleId)
        localSettings.value = JSON.parse(JSON.stringify(data))
        migrateLegacyVoiceModelFields()
        if (!localSettings.value.voice_lang) {
          localSettings.value.voice_lang = 'ja'
        }
      } catch (e) {
        console.error('Failed to load character settings', e)
        emit('close')
      } finally {
        loading.value = false
      }
    }
  },
)

// Refresh installed local voices whenever the voice tab is shown while the
// dialog is visible. The dropdown only matters when tts_type=localsbv2api,
// but loading early keeps things simple and the list is cheap to fetch.
watch(
  () => [props.visible, activeTab.value, localSettings.value.tts_type],
  ([visible, tab, ttsType]) => {
    if (visible && tab === 'voice' && ttsType === 'localsbv2api') {
      void refreshLocalVoices()
    }
  },
)

const REALTIME_SAVE_DEBOUNCE_MS = 300
let realtimeSaveTimer: ReturnType<typeof setTimeout> | null = null

const clearRealtimeSaveTimer = () => {
  if (realtimeSaveTimer !== null) {
    clearTimeout(realtimeSaveTimer)
    realtimeSaveTimer = null
  }
}

const handleClose = () => {
  clearRealtimeSaveTimer()
  emit('close')
}

const handleFieldChange = (field: FieldSchema) => {
  if (!field.realtime || !props.roleId) return

  const roleId = props.roleId
  clearRealtimeSaveTimer()
  realtimeSaveTimer = setTimeout(async () => {
    realtimeSaveTimer = null
    if (!props.visible || props.roleId !== roleId) return
    try {
      // 后端会同时保存 settings.yml 并重建已加载角色的 VoiceMaker。
      await updateRoleSettings(roleId, localSettings.value)
    } catch (e) {
      console.error(`实时更新 ${field.key} 失败:`, e)
      await dialogStore.alert(`实时更新 ${field.label} 失败，请检查控制台日志`)
    }
  }, REALTIME_SAVE_DEBOUNCE_MS)
}

const saveSettings = async () => {
  if (!props.roleId) return
  clearRealtimeSaveTimer()
  saving.value = true
  try {
    await updateRoleSettings(props.roleId, localSettings.value)
    emit('saved')
    emit('close')
  } catch (e) {
    console.error('Failed to save settings', e)
    await dialogStore.alert('保存失败，请检查控制台日志')
  } finally {
    saving.value = false
  }
}

onUnmounted(clearRealtimeSaveTimer)
</script>

<style scoped>
/* 表单控件 :focus 选中状态 */
/* Vertical sidebar: thin custom scrollbar (Webkit + Firefox). */
.tab-sidebar-scroll {
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.18) transparent;
}
.tab-sidebar-scroll::-webkit-scrollbar {
  width: 6px;
}
.tab-sidebar-scroll::-webkit-scrollbar-track {
  background: transparent;
}
.tab-sidebar-scroll::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.18);
  border-radius: 3px;
}
.tab-sidebar-scroll::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.32);
}

.form-control:focus {
  border-color: #79d9ff;
  background: rgba(0, 0, 0, 0.3);
  box-shadow: 0 0 0 3px rgba(121, 217, 255, 0.2);
}
</style>

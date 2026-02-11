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
          <!-- Sidebar -->
          <div class="w-48 bg-black/10 flex flex-col gap-2 p-4 border-r border-white/10">
            <button
              v-for="tab in tabs"
              :key="tab.id"
              class="w-full text-left px-4 py-2.5 rounded-xl border-none bg-transparent text-white/60 cursor-pointer transition-all duration-200 font-medium hover:bg-white/5 hover:text-white"
              :class="{
                'bg-[rgba(94,114,228,0.2)] !text-[#79d9ff] !font-semibold': activeTab === tab.id,
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
              <div v-if="currentTabConfig" class="space-y-4">
                <!-- Data-Driven Form Rendering -->
                <div
                  v-for="(field, index) in currentTabFields"
                  :key="index"
                  class="flex flex-col gap-2"
                >
                  <!-- Conditional Rendering wrapper -->
                  <template v-if="!field.visibleIf || field.visibleIf(localSettings)">
                    <label :for="field.key" class="text-[13px] text-white/60 font-medium"
                      >{{ field.label }} ({{ field.key }})</label
                    >

                    <!-- Text Input -->
                    <input
                      v-if="field.type === 'text' || field.type === 'number'"
                      :id="field.key"
                      v-model="fieldModel(field).value"
                      :type="field.type"
                      :step="field.step"
                      class="form-control bg-black/20 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm outline-none transition-all duration-200"
                    />

                    <!-- Textarea -->
                    <textarea
                      v-else-if="field.type === 'textarea'"
                      :id="field.key"
                      v-model="fieldModel(field).value"
                      :rows="field.rows || 4"
                      class="form-control bg-black/20 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm outline-none transition-all duration-200 font-mono leading-relaxed"
                    ></textarea>

                    <!-- Select -->
                    <select
                      v-else-if="field.type === 'select'"
                      :id="field.key"
                      v-model="fieldModel(field).value"
                      class="form-control bg-black/20 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm outline-none transition-all duration-200"
                    >
                      <option
                        v-for="opt in field.options"
                        :key="opt.value"
                        :value="opt.value"
                        class="bg-[#333] text-white"
                      >
                        {{ opt.label }}
                      </option>
                    </select>
                  </template>
                </div>

                <div
                  v-if="activeTab === 'voice' && voiceModelFields.length > 0"
                  class="p-4 bg-white/5 rounded-xl border border-white/10 space-y-4 mt-4"
                >
                  <h3 class="text-sm font-bold text-white/70 uppercase">声音模型配置</h3>
                  <div class="grid grid-cols-2 gap-4">
                    <div
                      v-for="(field, index) in voiceModelFields"
                      :key="'vm-' + index"
                      class="flex flex-col gap-2"
                    >
                      <label :for="field.key" class="text-[13px] text-white/60 font-medium">{{
                        field.key
                      }}</label>
                      <input
                        :id="field.key"
                        v-model="localSettings.voice_models[field.key]"
                        type="text"
                        class="form-control bg-black/20 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm outline-none transition-all duration-200"
                      />
                    </div>
                  </div>
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
import { ref, watch, computed } from 'vue'
import { getRoleSettings, updateRoleSettings } from '../../../api/services/character'
import { Icon } from '../../base'

const props = defineProps<{
  visible: boolean
  roleId: number | null
  title?: string
}>()

const emit = defineEmits(['close', 'saved'])

const activeTab = ref('basic')
const loading = ref(false)
const saving = ref(false)
const localSettings = ref<any>({})

const tabs = [
  { id: 'basic', label: '基本信息' },
  { id: 'prompts', label: '提示词' },
  { id: 'visuals', label: '视觉效果' },
  { id: 'voice', label: '语音设置' },
]

// --- Schema Definition ---

type FieldType = 'text' | 'number' | 'textarea' | 'select'

interface FieldSchema {
  key: string
  label: string
  type: FieldType
  rows?: number
  step?: string
  options?: { label: string; value: string }[]
  visibleIf?: (settings: any) => boolean
  isVoiceModel?: boolean
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
    { key: 'offset', label: '偏移', type: 'number' },
    { key: 'bubble_top', label: '气泡顶部距离', type: 'number' },
    { key: 'bubble_left', label: '气泡左侧距离', type: 'number' },
    { key: 'thinking_message', label: '思考消息文本', type: 'text' },
  ],
  voice: [
    {
      key: 'tts_type',
      label: 'TTS 类型',
      type: 'select',
      options: [
        { label: 'sva', value: 'sva' },
        { label: 'sbv2', value: 'sbv2' },
        { label: 'bv2', value: 'bv2' },
        { label: 'sbv2api', value: 'sbv2api' },
        { label: 'gsv', value: 'gsv' },
        { label: 'aivis', value: 'aivis' },
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
      visibleIf: (s) => s.tts_type === 'sbv2',
    },
    {
      key: 'sbv2_speaker_id',
      label: 'sbv2_speaker_id',
      type: 'text',
      isVoiceModel: true,
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
      visibleIf: (s) => s.tts_type === 'sbv2api',
    },
    {
      key: 'sbv2api_speaker_id',
      label: 'sbv2api_speaker_id',
      type: 'text',
      isVoiceModel: true,
      visibleIf: (s) => s.tts_type === 'sbv2api',
    },

    {
      key: 'gsv_voice_text',
      label: 'gsv_voice_text',
      type: 'text',
      isVoiceModel: true,
      visibleIf: (s) => s.tts_type === 'gsv',
    },
    {
      key: 'gsv_voice_filename',
      label: 'gsv_voice_filename',
      type: 'text',
      isVoiceModel: true,
      visibleIf: (s) => s.tts_type === 'gsv',
    },

    {
      key: 'aivis_model_uuid',
      label: 'aivis_model_uuid',
      type: 'text',
      isVoiceModel: true,
      visibleIf: (s) => s.tts_type === 'aivis',
    },
  ],
}

// --- Computed Properties ---

const currentTabConfig = computed(() => schemas[activeTab.value])

// voice model 单独处理
const currentTabFields = computed(() => {
  return (currentTabConfig.value || []).filter((f) => !f.isVoiceModel)
})

const voiceModelFields = computed(() => {
  if (activeTab.value !== 'voice') return []
  return (schemas.voice || []).filter(
    (f) => f.isVoiceModel && (f.visibleIf ? f.visibleIf(localSettings.value) : true),
  )
})

const fieldModel = (field: FieldSchema) => {
  return computed({
    get: () => {
      return localSettings.value[field.key]
    },
    set: (val) => {
      localSettings.value[field.key] = val
    },
  })
}

// --- Watchers & Methods ---

watch(
  () => props.visible,
  async (newVal) => {
    if (newVal && props.roleId) {
      loading.value = true
      try {
        const data = await getRoleSettings(props.roleId)
        localSettings.value = JSON.parse(JSON.stringify(data))

        if (!localSettings.value.voice_models) {
          localSettings.value.voice_models = {}
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

const handleClose = () => {
  emit('close')
}

const saveSettings = async () => {
  if (!props.roleId) return
  saving.value = true
  try {
    await updateRoleSettings(props.roleId, localSettings.value)
    emit('saved')
    emit('close')
  } catch (e) {
    console.error('Failed to save settings', e)
    alert('保存失败，请检查控制台日志')
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
/* 表单控件 :focus 选中状态 */
.form-control:focus {
  border-color: #79d9ff;
  background: rgba(0, 0, 0, 0.3);
  box-shadow: 0 0 0 3px rgba(121, 217, 255, 0.2);
}
</style>

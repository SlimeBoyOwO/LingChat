<template>
  <div class="character-settings-modal">
    <div class="modal-overlay" @click="closeModal"></div>
    <div class="modal-content">
      <div class="modal-header">
        <h3>{{ $t('settings.characterInfo.voice') }}</h3>
        <button @click="closeModal" class="close-btn">
          <X :size="20" />
        </button>
      </div>

      <div class="modal-body">
        <!-- TTS 类型选择 -->
        <div class="setting-group">
          <label class="setting-label">{{ $t('settings.characterInfo.fields.ttsType') }}</label>
          <select 
            v-model="voiceSettings.tts_type" 
            class="setting-select"
            @change="onTtsTypeChange"
          >
            <option value="sbv2">{{ $t('settings.characterInfo.fields.sbv2') }}</option>
            <option value="sherpa-onnx">{{ $t('settings.characterInfo.fields.sherpaOnnx') }}</option>
            <option value="opentts">{{ $t('settings.characterInfo.fields.opentts') }}</option>
            <option value="indextts2">{{ $t('settings.characterInfo.fields.indextts2') }}</option>
          </select>
        </div>

        <!-- Sherpa-ONNX 配置 -->
        <div v-if="voiceSettings.tts_type === 'sherpa-onnx'" class="sherpa-config">
          <h4 class="config-title">{{ $t('settings.characterInfo.fields.sherpaOnnxTitle') }}</h4>
          
          <!-- 模型选择 -->
          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.sherpaOnnxModelName') }}</label>
            <div class="model-list">
              <div v-for="model in availableModels" :key="model.id" class="model-item" :class="{ selected: voiceSettings.sherpa_onnx_model_name === model.id }">
                <div class="model-info" @click="voiceSettings.sherpa_onnx_model_name = model.id; onModelChange()">
                  <span class="model-name">{{ model.display_name }}</span>
                  <span class="model-meta">{{ model.model_type.toUpperCase() }} · {{ model.language }} · {{ formatSize(model.size_bytes) }}</span>
                </div>
                <div class="model-actions">
                  <button v-if="!model.installed" @click="downloadModel(model.id)" class="btn-icon btn-download" :disabled="downloading === model.id">
                    <Loader2 v-if="downloading === model.id" :size="14" class="spin" />
                    <Download v-else :size="14" />
                  </button>
                  <button v-else @click="deleteModel(model.id)" class="btn-icon btn-delete" :disabled="deleting === model.id">
                    <Loader2 v-if="deleting === model.id" :size="14" class="spin" />
                    <Trash2 v-else :size="14" />
                  </button>
                </div>
              </div>
              <p v-if="availableModels.length === 0" class="no-models">暂无可用模型</p>
            </div>
          </div>

          <!-- 模型路径 -->
          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.sherpaOnnxModelPath') }}</label>
            <input 
              v-model="voiceSettings.sherpa_onnx_model_path" 
              type="text" 
              class="setting-input"
              placeholder="模型文件路径"
              readonly
            />
            <button @click="openModelManager" class="btn-secondary">
              <FolderOpen :size="16" /> {{ $t('settings.characterInfo.fields.sherpaOnnxManageModels') }}
            </button>
          </div>

          <!-- 模型类型 -->
          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.sherpaOnnxModelType') }}</label>
            <select 
              v-model="voiceSettings.sherpa_onnx_model_type" 
              class="setting-select"
            >
              <option value="vits">VITS</option>
              <option value="fastspeech2">FastSpeech2</option>
              <option value="tortoise">Tortoise</option>
              <option value="matcha">Matcha-TTS</option>
            </select>
          </div>

          <!-- 语言选择 -->
          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.sherpaOnnxLanguage') }}</label>
            <select 
              v-model="voiceSettings.sherpa_onnx_lang" 
              class="setting-select"
            >
              <option value="zh">中文</option>
              <option value="en">English</option>
              <option value="ja">日本語</option>
              <option value="ko">한국어</option>
            </select>
          </div>

          <!-- 音色选择 -->
          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.sherpaOnnxVoice') }}</label>
            <select 
              v-model="voiceSettings.sherpa_onnx_voice" 
              class="setting-select"
            >
              <option value="female">{{ $t('settings.characterInfo.fields.sherpaOnnxVoicesFemale') }}</option>
              <option value="male">{{ $t('settings.characterInfo.fields.sherpaOnnxVoicesMale') }}</option>
              <option value="child">{{ $t('settings.characterInfo.fields.sherpaOnnxVoicesChild') }}</option>
              <option value="elderly">{{ $t('settings.characterInfo.fields.sherpaOnnxVoicesElderly') }}</option>
            </select>
          </div>

          <!-- GPU 加速 -->
          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.sherpaOnnxUseGpu') }}</label>
            <div class="toggle-switch">
              <input 
                v-model="voiceSettings.sherpa_onnx_use_gpu" 
                type="checkbox" 
                id="gpu-toggle"
                class="toggle-input"
              />
              <label for="gpu-toggle" class="toggle-label"></label>
            </div>
          </div>

          <!-- 模型参数 -->
          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.sherpaOnnxParameters') }}</label>
            <div class="parameter-grid">
              <div class="parameter-item">
                <label class="parameter-label">{{ $t('settings.characterInfo.fields.sherpaOnnxSpeed') }}</label>
                <input 
                  v-model.number="voiceSettings.sherpa_onnx_speed" 
                  type="range" 
                  min="0.5" 
                  max="2.0" 
                  step="0.1"
                  class="parameter-slider"
                />
                <span class="parameter-value">{{ voiceSettings.sherpa_onnx_speed }}</span>
              </div>
              <div class="parameter-item">
                <label class="parameter-label">{{ $t('settings.characterInfo.fields.sherpaOnnxPitch') }}</label>
                <input 
                  v-model.number="voiceSettings.sherpa_onnx_pitch" 
                  type="range" 
                  min="0.5" 
                  max="2.0" 
                  step="0.1"
                  class="parameter-slider"
                />
                <span class="parameter-value">{{ voiceSettings.sherpa_onnx_pitch }}</span>
              </div>
            </div>
          </div>

          <!-- 零样本克隆参考音频 -->
          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.sherpaOnnxRefAudio') }}</label>
            <div class="ref-audio-row">
              <input 
                :value="voiceSettings.sherpa_onnx_ref_audio_path" 
                type="text" 
                class="setting-input"
                placeholder="参考音频路径 (WAV)"
                readonly
              />
              <button @click="pickRefAudio" class="btn-secondary">
                <FolderOpen :size="16" /> 选择音频
              </button>
              <button v-if="voiceSettings.sherpa_onnx_ref_audio_path" @click="clearRefAudio" class="btn-secondary btn-danger">
                清除
              </button>
            </div>
            <p class="setting-hint">上传一段参考音频用于零样本声音克隆 (支持 WAV 格式)</p>
          </div>
          <div v-if="voiceSettings.sherpa_onnx_ref_audio_path" class="setting-group">
            <label class="setting-label">参考文本 (可选)</label>
            <input 
              v-model="voiceSettings.sherpa_onnx_ref_text" 
              type="text" 
              class="setting-input"
              placeholder="参考音频对应的文本内容"
            />
          </div>

          <!-- 测试按钮 -->
          <div class="setting-group">
            <button @click="testVoice" class="btn-primary">
              <MicVocal :size="16" /> {{ $t('settings.characterInfo.fields.test') }}
            </button>
          </div>
        </div>

        <!-- 其他 TTS 配置 -->
        <div v-else class="other-tts-config">
          <!-- 基础配置项可以根据不同的 TTS 类型显示 -->
          <div v-if="voiceSettings.tts_type === 'sbv2'" class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.speakerId') }}</label>
            <input 
              v-model="voiceSettings.sbv2_speaker_id" 
              type="text" 
              class="setting-input"
              placeholder="说话人ID"
            />
          </div>

          <div v-if="voiceSettings.tts_type === 'opentts'" class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.openttsVoice') }}</label>
            <input 
              v-model="voiceSettings.opentts_voice" 
              type="text" 
              class="setting-input"
              placeholder="OpenTTS voice ID"
            />
          </div>
        </div>

        <!-- 基础语音设置 -->
        <div class="basic-settings">
          <h4 class="config-title">{{ $t('settings.characterInfo.fields.basicSettings') }}</h4>
          
          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.voiceLang') }}</label>
            <select 
              v-model="voiceSettings.voice_lang" 
              class="setting-select"
            >
              <option value="ja">日本語</option>
              <option value="zh">中文</option>
              <option value="en">English</option>
              <option value="ko">한국어</option>
            </select>
          </div>

          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.emotion') }}</label>
            <select 
              v-model="voiceSettings.emotion" 
              class="setting-select"
            >
              <option value="normal">{{ $t('settings.characterInfo.fields.emotionsNormal') }}</option>
              <option value="happy">{{ $t('settings.characterInfo.fields.emotionsHappy') }}</option>
              <option value="sad">{{ $t('settings.characterInfo.fields.emotionsSad') }}</option>
              <option value="angry">{{ $t('settings.characterInfo.fields.emotionsAngry') }}</option>
              <option value="surprised">{{ $t('settings.characterInfo.fields.emotionsSurprised') }}</option>
            </select>
          </div>
        </div>
      </div>

      <div class="modal-footer">
        <button @click="closeModal" class="btn-secondary">{{ $t('settings.shared.cancel') }}</button>
        <button @click="saveSettings" class="btn-primary">{{ $t('settings.shared.save') }}</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { X, FolderOpen, MicVocal, Download, Trash2, Loader2 } from 'lucide-vue-next'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { listSherpaModels, downloadSherpaModel, deleteSherpaModel, type SherpaOnnxModelRecord } from '@/api/services/tts/tts-local'

interface VoiceSettings {
  tts_type: string
  voice_lang: string
  emotion: string
  // Sherpa-ONNX 配置
  sherpa_onnx_model_name: string
  sherpa_onnx_model_path: string
  sherpa_onnx_model_type: string
  sherpa_onnx_lang: string
  sherpa_onnx_voice: string
  sherpa_onnx_use_gpu: boolean
  sherpa_onnx_speed: number
  sherpa_onnx_pitch: number
  sherpa_onnx_ref_audio_path: string
  sherpa_onnx_ref_text: string
  // 其他 TTS 配置
  sbv2_speaker_id: string
  opentts_voice: string
}

const props = defineProps<{
  characterId: number
  initialSettings: any
}>()

const emit = defineEmits<{
  close: []
  save: [settings: VoiceSettings]
}>()

const { t } = useI18n()

const voiceSettings = ref<VoiceSettings>({
  tts_type: 'sbv2',
  voice_lang: 'ja',
  emotion: 'normal',
  // Sherpa-ONNX 配置
  sherpa_onnx_model_name: '',
  sherpa_onnx_model_path: '',
  sherpa_onnx_model_type: 'vits',
  sherpa_onnx_lang: 'zh',
  sherpa_onnx_voice: 'female',
  sherpa_onnx_use_gpu: false,
  sherpa_onnx_speed: 1.0,
  sherpa_onnx_pitch: 1.0,
  sherpa_onnx_ref_audio_path: '',
  sherpa_onnx_ref_text: '',
  // 其他 TTS 配置
  sbv2_speaker_id: '',
  opentts_voice: ''
})

const availableModels = ref<SherpaOnnxModelRecord[]>([])
const downloading = ref<string | null>(null)
const deleting = ref<string | null>(null)

// 初始化设置
onMounted(() => {
  if (props.initialSettings) {
    voiceSettings.value = { ...voiceSettings.value, ...props.initialSettings }
  }
  loadAvailableModels()
})

// 加载可用的模型
const loadAvailableModels = async () => {
  try {
    availableModels.value = await listSherpaModels()
  } catch (error) {
    console.error('加载模型列表失败:', error)
    availableModels.value = []
  }
}

// 下载模型
const downloadModel = async (modelId: string) => {
  downloading.value = modelId
  try {
    await downloadSherpaModel(modelId)
    await loadAvailableModels()
  } catch (error) {
    console.error('下载模型失败:', error)
  } finally {
    downloading.value = null
  }
}

// 删除模型
const deleteModel = async (modelId: string) => {
  deleting.value = modelId
  try {
    await deleteSherpaModel(modelId)
    if (voiceSettings.value.sherpa_onnx_model_name === modelId) {
      voiceSettings.value.sherpa_onnx_model_name = ''
      voiceSettings.value.sherpa_onnx_model_path = ''
    }
    await loadAvailableModels()
  } catch (error) {
    console.error('删除模型失败:', error)
  } finally {
    deleting.value = null
  }
}

// TTS 类型改变
const onTtsTypeChange = () => {
  // 重置 Sherpa-ONNX 配置
  if (voiceSettings.value.tts_type !== 'sherpa-onnx') {
    voiceSettings.value.sherpa_onnx_model_name = ''
    voiceSettings.value.sherpa_onnx_model_path = ''
  }
}

// 模型改变
const onModelChange = () => {
  const selectedModel = availableModels.value.find(model => model.id === voiceSettings.value.sherpa_onnx_model_name)
  if (selectedModel) {
    voiceSettings.value.sherpa_onnx_model_path = selectedModel.path
    voiceSettings.value.sherpa_onnx_lang = selectedModel.language
    voiceSettings.value.sherpa_onnx_model_type = selectedModel.model_type
    voiceSettings.value.sherpa_onnx_voice = selectedModel.voice
  }
}

// 打开模型管理器
const openModelManager = async () => {
  try {
    await invoke('open_sherpa_onnx_model_manager')
  } catch (error) {
    console.error('打开模型管理器失败:', error)
  }
}

// 测试语音
const testVoice = async () => {
  try {
    await invoke('test_sherpa_onnx_voice', {
      text: '你好，这是一个测试。',
      settings: voiceSettings.value
    })
  } catch (error) {
    console.error('语音测试失败:', error)
  }
}

// 选择参考音频
const pickRefAudio = async () => {
  try {
    const file = await open({
      multiple: false,
      filters: [{ name: 'Audio', extensions: ['wav', 'mp3', 'ogg', 'flac'] }]
    })
    if (file) {
      voiceSettings.value.sherpa_onnx_ref_audio_path = file as string
    }
  } catch (error) {
    console.error('选择参考音频失败:', error)
  }
}

// 清除参考音频
const clearRefAudio = () => {
  voiceSettings.value.sherpa_onnx_ref_audio_path = ''
  voiceSettings.value.sherpa_onnx_ref_text = ''
}

// 保存设置
const saveSettings = () => {
  emit('save', voiceSettings.value)
  closeModal()
}

// 关闭模态框
const closeModal = () => {
  emit('close')
}

// 格式化文件大小
const formatSize = (bytes: number): string => {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}
</script>

<style scoped>
.character-settings-modal {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(4px);
}

.modal-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
}

.modal-content {
  position: relative;
  background: rgba(30, 30, 46, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 16px;
  width: 90%;
  max-width: 600px;
  max-height: 80vh;
  overflow-y: auto;
  margin: 20px;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.modal-header h3 {
  color: white;
  font-size: 18px;
  font-weight: 600;
  margin: 0;
}

.close-btn {
  background: none;
  border: none;
  color: white;
  cursor: pointer;
  padding: 4px;
  border-radius: 4px;
  transition: background-color 0.2s;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.1);
}

.modal-body {
  padding: 24px;
}

.setting-group {
  margin-bottom: 20px;
}

.setting-label {
  display: block;
  color: rgba(255, 255, 255, 0.8);
  font-size: 14px;
  font-weight: 500;
  margin-bottom: 8px;
}

.setting-select,
.setting-input {
  width: 100%;
  padding: 10px 12px;
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 8px;
  color: white;
  font-size: 14px;
  transition: border-color 0.2s;
}

.setting-select:focus,
.setting-input:focus {
  outline: none;
  border-color: rgba(99, 102, 241, 0.5);
}

.setting-input:read-only {
  background: rgba(255, 255, 255, 0.05);
  cursor: not-allowed;
}

.config-title {
  color: white;
  font-size: 16px;
  font-weight: 600;
  margin: 24px 0 16px 0;
  padding-bottom: 8px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.sherpa-config {
  background: rgba(99, 102, 241, 0.1);
  border: 1px solid rgba(99, 102, 241, 0.2);
  border-radius: 12px;
  padding: 16px;
  margin-bottom: 24px;
}

.toggle-switch {
  display: flex;
  align-items: center;
  gap: 12px;
}

.toggle-input {
  display: none;
}

.toggle-label {
  position: relative;
  width: 48px;
  height: 24px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 12px;
  cursor: pointer;
  transition: background-color 0.2s;
}

.toggle-label::before {
  content: '';
  position: absolute;
  top: 2px;
  left: 2px;
  width: 20px;
  height: 20px;
  background: white;
  border-radius: 50%;
  transition: transform 0.2s;
}

.toggle-input:checked + .toggle-label {
  background: rgba(99, 102, 241, 0.5);
}

.toggle-input:checked + .toggle-label::before {
  transform: translateX(24px);
}

.parameter-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.parameter-item {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.parameter-label {
  color: rgba(255, 255, 255, 0.8);
  font-size: 12px;
}

.parameter-slider {
  width: 100%;
  height: 6px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 3px;
  outline: none;
  -webkit-appearance: none;
}

.parameter-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 16px;
  height: 16px;
  background: rgba(99, 102, 241, 0.8);
  border-radius: 50%;
  cursor: pointer;
}

.parameter-value {
  color: rgba(255, 255, 255, 0.6);
  font-size: 12px;
  text-align: center;
}

.btn-primary,
.btn-secondary {
  padding: 10px 16px;
  border: none;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.btn-primary {
  background: rgba(99, 102, 241, 0.8);
  color: white;
}

.btn-primary:hover {
  background: rgba(99, 102, 241, 1);
}

.btn-secondary {
  background: rgba(255, 255, 255, 0.1);
  color: white;
  border: 1px solid rgba(255, 255, 255, 0.2);
}

.btn-secondary:hover {
  background: rgba(255, 255, 255, 0.2);
}

.btn-danger {
  background: rgba(239, 68, 68, 0.2);
  border-color: rgba(239, 68, 68, 0.4);
}

.btn-danger:hover {
  background: rgba(239, 68, 68, 0.4);
}

.ref-audio-row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.ref-audio-row .setting-input {
  flex: 1;
}

.model-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.model-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  transition: all 0.2s;
  cursor: pointer;
}

.model-item:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(99, 102, 241, 0.3);
}

.model-item.selected {
  background: rgba(99, 102, 241, 0.15);
  border-color: rgba(99, 102, 241, 0.5);
}

.model-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
}

.model-name {
  color: white;
  font-size: 14px;
  font-weight: 500;
}

.model-meta {
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
}

.model-actions {
  display: flex;
  gap: 4px;
}

.btn-icon {
  background: none;
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: white;
  cursor: pointer;
  padding: 6px;
  border-radius: 6px;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.btn-icon:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-download {
  border-color: rgba(34, 197, 94, 0.4);
}

.btn-download:hover:not(:disabled) {
  background: rgba(34, 197, 94, 0.2);
  border-color: rgba(34, 197, 94, 0.6);
}

.btn-delete {
  border-color: rgba(239, 68, 68, 0.4);
}

.btn-delete:hover:not(:disabled) {
  background: rgba(239, 68, 68, 0.2);
  border-color: rgba(239, 68, 68, 0.6);
}

.no-models {
  color: rgba(255, 255, 255, 0.4);
  font-size: 13px;
  text-align: center;
  padding: 12px;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.setting-hint {
  margin-top: 4px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  padding: 20px 24px;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
}

@media (max-width: 640px) {
  .modal-content {
    width: 95%;
    margin: 10px;
  }

  .parameter-grid {
    grid-template-columns: 1fr;
  }
}
</style>
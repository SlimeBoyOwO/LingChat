<template>
  <div class="character-settings-modal">
    <div class="modal-overlay" @click="closeModal" />
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

        <!-- Sherpa-ONNX: 打开独立配置面板 -->
        <div v-if="voiceSettings.tts_type === 'sherpa-onnx'" class="sherpa-entry">
          <div class="sherpa-entry-info">
            <Cpu :size="18" class="sherpa-entry-icon" />
            <div>
              <p class="sherpa-entry-title">Sherpa-ONNX 本地语音</p>
              <p class="sherpa-entry-desc">模型管理、参数调节、零样本声音克隆</p>
            </div>
          </div>
          <button @click="openSherpaModal" class="btn-primary btn-sm">
            <Settings :size="14" /> 配置
          </button>
        </div>

        <!-- 其他 TTS 配置 -->
        <div v-else class="other-tts-config">
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
            <select v-model="voiceSettings.voice_lang" class="setting-select">
              <option value="ja">日本語</option>
              <option value="zh">中文</option>
              <option value="en">English</option>
              <option value="ko">한국어</option>
            </select>
          </div>

          <div class="setting-group">
            <label class="setting-label">{{ $t('settings.characterInfo.fields.emotion') }}</label>
            <select v-model="voiceSettings.emotion" class="setting-select">
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

    <!-- Sherpa-ONNX 独立配置弹窗 -->
    <SherpaOnnxSettingsModal
      v-if="showSherpaModal"
      :character-id="characterId"
      :initial-settings="sherpaSettings"
      @close="showSherpaModal = false"
      @save="onSherpaSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { X, Cpu, Settings } from 'lucide-vue-next'
import SherpaOnnxSettingsModal from './SherpaOnnxSettingsModal.vue'
import type { SherpaOnnxSettings } from './SherpaOnnxSettingsModal.vue'

interface VoiceSettings {
  tts_type: string
  voice_lang: string
  emotion: string
  // Sherpa-ONNX 配置 (由独立弹窗管理)
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
  sbv2_speaker_id: '',
  opentts_voice: '',
})

const showSherpaModal = ref(false)

const sherpaSettings = ref<SherpaOnnxSettings>({
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
})

onMounted(() => {
  if (props.initialSettings) {
    voiceSettings.value = { ...voiceSettings.value, ...props.initialSettings }
    // 同步 Sherpa-ONNX 字段到独立设置
    sherpaSettings.value = {
      sherpa_onnx_model_name: voiceSettings.value.sherpa_onnx_model_name,
      sherpa_onnx_model_path: voiceSettings.value.sherpa_onnx_model_path,
      sherpa_onnx_model_type: voiceSettings.value.sherpa_onnx_model_type,
      sherpa_onnx_lang: voiceSettings.value.sherpa_onnx_lang,
      sherpa_onnx_voice: voiceSettings.value.sherpa_onnx_voice,
      sherpa_onnx_use_gpu: voiceSettings.value.sherpa_onnx_use_gpu,
      sherpa_onnx_speed: voiceSettings.value.sherpa_onnx_speed,
      sherpa_onnx_pitch: voiceSettings.value.sherpa_onnx_pitch,
      sherpa_onnx_ref_audio_path: voiceSettings.value.sherpa_onnx_ref_audio_path,
      sherpa_onnx_ref_text: voiceSettings.value.sherpa_onnx_ref_text,
    }
  }
})

const onTtsTypeChange = () => {
  if (voiceSettings.value.tts_type !== 'sherpa-onnx') {
    voiceSettings.value.sherpa_onnx_model_name = ''
    voiceSettings.value.sherpa_onnx_model_path = ''
  }
}

const openSherpaModal = () => {
  showSherpaModal.value = true
}

const onSherpaSaved = (settings: SherpaOnnxSettings) => {
  // 将 Sherpa-ONNX 设置同步回主设置
  Object.assign(voiceSettings.value, settings)
  showSherpaModal.value = false
}

const saveSettings = () => {
  emit('save', voiceSettings.value)
  closeModal()
}

const closeModal = () => {
  emit('close')
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
  max-width: 500px;
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

.config-title {
  color: white;
  font-size: 16px;
  font-weight: 600;
  margin: 24px 0 16px 0;
  padding-bottom: 8px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

/* Sherpa-ONNX entry card */
.sherpa-entry {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  background: rgba(139, 92, 246, 0.08);
  border: 1px solid rgba(139, 92, 246, 0.2);
  border-radius: 12px;
  margin-bottom: 20px;
  gap: 12px;
}

.sherpa-entry-info {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
  min-width: 0;
}

.sherpa-entry-info > div {
  min-width: 0;
}

.sherpa-entry-icon {
  color: rgba(139, 92, 246, 0.8);
}

.sherpa-entry-title {
  color: white;
  font-size: 14px;
  font-weight: 500;
  margin: 0;
}

.sherpa-entry-desc {
  color: rgba(255, 255, 255, 0.45);
  font-size: 12px;
  margin: 2px 0 0 0;
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

.btn-sm {
  padding: 7px 14px;
  font-size: 13px;
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
  .modal-body {
    padding: 16px;
  }
  .modal-header {
    padding: 16px;
  }
  .modal-footer {
    padding: 16px;
  }
  .sherpa-entry {
    flex-direction: column;
    align-items: flex-start;
  }
  .sherpa-entry .btn-sm {
    width: 100%;
    justify-content: center;
  }
  .sherpa-entry-desc {
    white-space: normal;
  }
}
</style>

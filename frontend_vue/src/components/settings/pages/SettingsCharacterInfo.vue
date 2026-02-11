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
              <h2 class="text-xl font-bold m-0 text-shadow">{{ title }} - 配置编辑</h2>
              <p class="text-sm text-white/50 m-0">修改角色的详细设置</p>
            </div>
          </div>
          <button class="close-btn" @click="handleClose">
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
              class="tab-btn"
              :class="{ active: activeTab === tab.id }"
              @click="activeTab = tab.id"
            >
              {{ tab.label }}
            </button>
          </div>

          <!-- Tab Panels -->
          <div class="flex-1 overflow-y-auto p-6 relative">
            <div v-if="loading" class="flex items-center justify-center h-full">
              <div class="spinner"></div>
            </div>

            <div v-else class="max-w-3xl mx-auto space-y-6">
              <!-- Basic Info Panel -->
              <div v-if="activeTab === 'basic'" class="space-y-4 animate-fade-in">
                <div class="form-group">
                  <label>AI 名称（ai_name）</label>
                  <input v-model="localSettings.ai_name" type="text" class="form-input" />
                </div>
                <div class="form-group">
                  <label>AI 副标题（ai_subtitle）</label>
                  <input v-model="localSettings.ai_subtitle" type="text" class="form-input" />
                </div>
                <div class="form-group">
                  <label>用户名称（user_name）</label>
                  <input v-model="localSettings.user_name" type="text" class="form-input" />
                </div>
                <div class="form-group">
                  <label>用户副标题（user_subtitle）</label>
                  <input v-model="localSettings.user_subtitle" type="text" class="form-input" />
                </div>
                <div class="form-group">
                  <label>角色标题（title）</label>
                  <input v-model="localSettings.title" type="text" class="form-input" />
                </div>
                <div class="form-group">
                  <label>角色介绍（info）</label>
                  <textarea v-model="localSettings.info" rows="4" class="form-textarea"></textarea>
                </div>
              </div>

              <!-- Prompts Panel -->
              <div v-if="activeTab === 'prompts'" class="space-y-4 animate-fade-in">
                <div class="form-group">
                  <label>系统提示词（system_prompt）</label>
                  <textarea
                    v-model="localSettings.system_prompt"
                    rows="10"
                    class="form-textarea font-mono text-sm leading-relaxed"
                  ></textarea>
                </div>
                <div class="form-group">
                  <label>对话示例（system_prompt_example）</label>
                  <textarea
                    v-model="localSettings.system_prompt_example"
                    rows="6"
                    class="form-textarea font-mono text-sm"
                  ></textarea>
                </div>
                <div class="form-group">
                  <label>旧版兼容对话示例（system_prompt_example_old）</label>
                  <textarea
                    v-model="localSettings.system_prompt_example_old"
                    rows="4"
                    class="form-textarea font-mono text-sm"
                  ></textarea>
                </div>
              </div>

              <!-- Visuals Panel -->
              <div v-if="activeTab === 'visuals'" class="space-y-4 animate-fade-in">
                <div class="grid grid-cols-2 gap-4">
                  <div class="form-group">
                    <label>缩放（scale）</label>
                    <input
                      v-model.number="localSettings.scale"
                      type="number"
                      step="0.01"
                      class="form-input"
                    />
                  </div>
                  <div class="form-group">
                    <label>偏移（offset）</label>
                    <input v-model.number="localSettings.offset" type="number" class="form-input" />
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4">
                  <div class="form-group">
                    <label>气泡顶部距离（bubble_top）</label>
                    <input
                      v-model.number="localSettings.bubble_top"
                      type="number"
                      class="form-input"
                    />
                  </div>
                  <div class="form-group">
                    <label>气泡左侧距离（bubble_left）</label>
                    <input
                      v-model.number="localSettings.bubble_left"
                      type="number"
                      class="form-input"
                    />
                  </div>
                </div>

                <div class="form-group">
                  <label>思考消息文本（thinking_message）</label>
                  <input v-model="localSettings.thinking_message" type="text" class="form-input" />
                </div>
              </div>

              <!-- Voice Panel -->
              <div v-if="activeTab === 'voice'" class="space-y-4 animate-fade-in">
                <div class="form-group">
                  <label>TTS 类型（tts_type）</label>
                  <select v-model="localSettings.tts_type" class="form-select">
                    <option value="sva">sva</option>
                    <option value="sbv2">sbv2</option>
                    <option value="bv2">bv2</option>
                    <option value="sbv2api">sbv2api</option>
                    <option value="gsv">gsv</option>
                    <option value="aivis">aivis</option>
                  </select>
                </div>

                <div class="p-4 bg-white/5 rounded-xl border border-white/10 space-y-4">
                  <h3 class="text-sm font-bold text-white/70 uppercase">声音模型配置</h3>

                  <div class="grid grid-cols-2 gap-4">
                    <!-- sva -->
                    <div v-if="localSettings.tts_type === 'sva'" class="form-group">
                      <label>sva_speaker_id</label>
                      <input
                        v-model="voiceModelsProxy.sva_speaker_id"
                        type="text"
                        class="form-input"
                      />
                    </div>

                    <!-- sbv2 -->
                    <template v-if="localSettings.tts_type === 'sbv2'">
                      <div class="form-group">
                        <label>sbv2_name</label>
                        <input
                          v-model="voiceModelsProxy.sbv2_name"
                          type="text"
                          class="form-input"
                        />
                      </div>
                      <div class="form-group">
                        <label>sbv2_speaker_id</label>
                        <input
                          v-model="voiceModelsProxy.sbv2_speaker_id"
                          type="text"
                          class="form-input"
                        />
                      </div>
                    </template>

                    <!-- bv2 -->
                    <div v-if="localSettings.tts_type === 'bv2'" class="form-group">
                      <label>bv2_speaker_id</label>
                      <input
                        v-model="voiceModelsProxy.bv2_speaker_id"
                        type="text"
                        class="form-input"
                      />
                    </div>

                    <!-- sbv2api -->
                    <template v-if="localSettings.tts_type === 'sbv2api'">
                      <div class="form-group">
                        <label>sbv2api_name</label>
                        <input
                          v-model="voiceModelsProxy.sbv2api_name"
                          type="text"
                          class="form-input"
                        />
                      </div>
                      <div class="form-group">
                        <label>sbv2api_speaker_id</label>
                        <input
                          v-model="voiceModelsProxy.sbv2api_speaker_id"
                          type="text"
                          class="form-input"
                        />
                      </div>
                    </template>

                    <!-- gsv -->
                    <template v-if="localSettings.tts_type === 'gsv'">
                      <div class="form-group">
                        <label>gsv_voice_text</label>
                        <input
                          v-model="voiceModelsProxy.gsv_voice_text"
                          type="text"
                          class="form-input"
                        />
                      </div>
                      <div class="form-group">
                        <label>gsv_voice_filename</label>
                        <input
                          v-model="voiceModelsProxy.gsv_voice_filename"
                          type="text"
                          class="form-input"
                        />
                      </div>
                    </template>

                    <!-- aivis -->
                    <div v-if="localSettings.tts_type === 'aivis'" class="form-group">
                      <label>aivis_model_uuid</label>
                      <input
                        v-model="voiceModelsProxy.aivis_model_uuid"
                        type="text"
                        class="form-input"
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
          <button class="footer-btn secondary" @click="handleClose">取消</button>
          <button class="footer-btn primary" :disabled="saving" @click="saveSettings">
            <span v-if="saving" class="spinner-sm mr-2"></span>
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

// Proxy for voice_models to handle null/undefined gracefully
const voiceModelsProxy = computed(() => {
  if (!localSettings.value.voice_models) {
    localSettings.value.voice_models = {}
  }
  return localSettings.value.voice_models
})

watch(
  () => props.visible,
  async (newVal) => {
    if (newVal && props.roleId) {
      loading.value = true
      try {
        const data = await getRoleSettings(props.roleId)
        localSettings.value = JSON.parse(JSON.stringify(data)) // Deep copy
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
/* Modal Transition */
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.3s ease;
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

/* Custom Styles */
.text-shadow {
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

.close-btn {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  border: none;
  background: rgba(255, 255, 255, 0.1);
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.2);
  transform: rotate(90deg);
}

.tab-btn {
  width: 100%;
  text-align: left;
  padding: 10px 16px;
  border-radius: 12px;
  border: none;
  background: transparent;
  color: rgba(255, 255, 255, 0.6);
  cursor: pointer;
  transition: all 0.2s;
  font-weight: 500;
}

.tab-btn:hover {
  background: rgba(255, 255, 255, 0.05);
  color: white;
}

.tab-btn.active {
  background: rgba(94, 114, 228, 0.2);
  color: #79d9ff;
  font-weight: 600;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.form-group label {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.6);
  font-weight: 500;
}

.form-input,
.form-select,
.form-textarea {
  background: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 10px 14px;
  color: white;
  font-size: 14px;
  outline: none;
  transition: all 0.2s;
}

.form-input:focus,
.form-select:focus,
.form-textarea:focus {
  border-color: #5e72e4;
  background: rgba(0, 0, 0, 0.3);
  box-shadow: 0 0 0 3px rgba(94, 114, 228, 0.2);
}

.footer-btn {
  padding: 8px 20px;
  border-radius: 20px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  border: none;
}

.footer-btn.secondary {
  background: rgba(255, 255, 255, 0.1);
  color: white;
}

.footer-btn.secondary:hover {
  background: rgba(255, 255, 255, 0.2);
}

.footer-btn.primary {
  background: #5e72e4;
  color: white;
}

.footer-btn.primary:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.footer-btn.primary:not(:disabled):hover {
  background: #4a5acf;
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(94, 114, 228, 0.3);
}

.spinner {
  width: 40px;
  height: 40px;
  border: 3px solid rgba(255, 255, 255, 0.1);
  border-top-color: #5e72e4;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

.spinner-sm {
  display: inline-block;
  width: 14px;
  height: 14px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.animate-fade-in {
  animation: fadeIn 0.3s ease;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>

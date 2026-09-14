<template>
  <Teleport to="body">
    <div class="sherpa-modal">
      <div class="modal-overlay" @click="closeModal" />
      <div class="modal-content">
        <div class="modal-header">
          <div class="header-left">
            <Cpu :size="20" class="header-icon" />
            <h3>{{ $t("settings.characterInfo.fields.sherpaOnnx") }}</h3>
          </div>
          <button @click="closeModal" class="close-btn">
            <X :size="20" />
          </button>
        </div>

        <div class="modal-body">
          <!-- 模型选择 -->
          <section class="config-section">
            <h4 class="section-title">
              <Package :size="16" />
              {{ $t("settings.characterInfo.fields.sherpaOnnxModelName") }}
            </h4>

            <!-- Android 存储权限未授权警告 -->
            <div v-if="isAndroid() && !storageGranted" class="perm-warning">
              <AlertTriangle :size="16" class="perm-warning-icon" />
              <div class="perm-warning-text">
                <p>{{ $t("settings.characterInfo.fields.sherpaOnnxStoragePermHint") }}</p>
                <button @click="requestStoragePermission" class="btn-secondary btn-sm">
                  <ShieldCheck :size="14" />
                  {{ $t("settings.characterInfo.fields.sherpaOnnxGrantPermission") }}
                </button>
              </div>
            </div>

            <!-- 模型存放目录 -->
            <div class="model-dir-info">
              <FolderOpen :size="14" class="model-dir-icon" />
              <span class="model-dir-label">{{
                $t("settings.characterInfo.fields.sherpaOnnxModelDir")
              }}</span>
              <code class="model-dir-path">{{ modelRoot || "…" }}</code>
            </div>

            <div class="model-list">
              <div
                v-for="model in availableModels"
                :key="model.id"
                class="model-item"
                :class="{ selected: settings.sherpa_onnx_model_name === model.id }"
                @click="selectModel(model)"
              >
                <div class="model-info">
                  <span class="model-name">{{ model.display_name }}</span>
                  <span class="model-meta">
                    {{ model.model_type.toUpperCase() }} · {{ model.language }} ·
                    {{ formatSize(model.size_bytes) }}
                  </span>
                </div>
                <div class="model-actions">
                  <button
                    v-if="!model.installed"
                    @click.stop="downloadModel(model.id)"
                    class="btn-icon btn-download"
                    :disabled="downloading === model.id"
                  >
                    <Loader2 v-if="downloading === model.id" :size="14" class="spin" />
                    <Download v-else :size="14" />
                  </button>
                  <button
                    v-else
                    @click.stop="deleteModel(model.id)"
                    class="btn-icon btn-delete"
                    :disabled="deleting === model.id"
                  >
                    <Loader2 v-if="deleting === model.id" :size="14" class="spin" />
                    <Trash2 v-else :size="14" />
                  </button>
                </div>
                <div v-if="downloading === model.id" class="model-progress">
                  <div class="progress-track">
                    <div
                      class="progress-fill"
                      :style="{ width: Math.round(progressByModel[model.id] ?? 0) + '%' }"
                    />
                  </div>
                  <span class="progress-text"
                    >{{ Math.round(progressByModel[model.id] ?? 0) }}%</span
                  >
                </div>
              </div>
              <p v-if="availableModels.length === 0" class="no-models">暂无可用模型</p>
            </div>
            <button @click="openModelManager" class="btn-secondary btn-block">
              <FolderOpen :size="16" />
              {{ $t("settings.characterInfo.fields.sherpaOnnxManageModels") }}
            </button>
          </section>

          <!-- 模型配置 -->
          <section class="config-section">
            <h4 class="section-title">
              <Settings :size="16" />
              模型配置
            </h4>

            <div class="config-grid">
              <div class="config-item">
                <label class="config-label">{{
                  $t("settings.characterInfo.fields.sherpaOnnxModelType")
                }}</label>
                <select v-model="settings.sherpa_onnx_model_type" class="config-select">
                  <option value="vits">VITS</option>
                  <option value="fastspeech2">FastSpeech2</option>
                  <option value="matcha">Matcha-TTS</option>
                  <option value="kokoro">Kokoro</option>
                  <option value="kitten">Kitten</option>
                  <option value="zipvoice">ZipVoice</option>
                  <option value="pocket">Pocket TTS</option>
                  <option value="supertonic">Supertonic</option>
                </select>
              </div>

              <div class="config-item">
                <label class="config-label">{{
                  $t("settings.characterInfo.fields.sherpaOnnxLanguage")
                }}</label>
                <select v-model="settings.sherpa_onnx_lang" class="config-select">
                  <option value="zh">中文</option>
                  <option value="en">English</option>
                  <option value="ja">日本語</option>
                  <option value="ko">한국어</option>
                </select>
              </div>

              <div class="config-item">
                <label class="config-label">{{
                  $t("settings.characterInfo.fields.sherpaOnnxVoice")
                }}</label>
                <select v-model="settings.sherpa_onnx_voice" class="config-select">
                  <option value="female">
                    {{ $t("settings.characterInfo.fields.sherpaOnnxVoicesFemale") }}
                  </option>
                  <option value="male">
                    {{ $t("settings.characterInfo.fields.sherpaOnnxVoicesMale") }}
                  </option>
                  <option value="child">
                    {{ $t("settings.characterInfo.fields.sherpaOnnxVoicesChild") }}
                  </option>
                  <option value="elderly">
                    {{ $t("settings.characterInfo.fields.sherpaOnnxVoicesElderly") }}
                  </option>
                </select>
              </div>

              <div class="config-item">
                <label class="config-label">{{
                  $t("settings.characterInfo.fields.sherpaOnnxUseGpu")
                }}</label>
                <label class="toggle-wrapper">
                  <input
                    v-model="settings.sherpa_onnx_use_gpu"
                    type="checkbox"
                    class="toggle-input"
                  />
                  <span class="toggle-track">
                    <span class="toggle-thumb" />
                  </span>
                </label>
              </div>
            </div>
          </section>

          <!-- 参数调节 -->
          <section class="config-section">
            <h4 class="section-title">
              <SlidersHorizontal :size="16" />
              {{ $t("settings.characterInfo.fields.sherpaOnnxParameters") }}
            </h4>
            <div class="param-grid">
              <div class="param-item">
                <div class="param-header">
                  <label class="param-label">{{
                    $t("settings.characterInfo.fields.sherpaOnnxSpeed")
                  }}</label>
                  <span class="param-value">{{ settings.sherpa_onnx_speed.toFixed(1) }}</span>
                </div>
                <input
                  v-model.number="settings.sherpa_onnx_speed"
                  type="range"
                  min="0.5"
                  max="2.0"
                  step="0.1"
                  class="param-slider"
                />
              </div>
            </div>
          </section>

          <!-- 零样本合成 -->
          <section class="config-section zero-shot-section">
            <h4 class="section-title">
              <Wand2 :size="16" />
              零样本声音克隆
            </h4>
            <p class="section-desc">上传一段参考音频，模型将模仿该声音的音色进行合成</p>

            <div class="ref-audio-card" :class="{ active: settings.sherpa_onnx_ref_audio_path }">
              <div class="ref-audio-info">
                <Mic :size="24" class="ref-audio-icon" />
                <div v-if="settings.sherpa_onnx_ref_audio_path" class="ref-audio-detail">
                  <span class="ref-audio-name">{{ refAudioName }}</span>
                  <span class="ref-audio-status">已加载参考音频</span>
                </div>
                <div v-else class="ref-audio-detail">
                  <span class="ref-audio-name">未选择音频</span>
                  <span class="ref-audio-status">仅支持 WAV 格式</span>
                </div>
              </div>
              <div class="ref-audio-actions">
                <button @click="pickRefAudio" class="btn-secondary">
                  <FolderOpen :size="16" />
                  {{ settings.sherpa_onnx_ref_audio_path ? "更换" : "选择音频" }}
                </button>
                <button
                  v-if="settings.sherpa_onnx_ref_audio_path"
                  @click="clearRefAudio"
                  class="btn-secondary btn-danger"
                >
                  <X :size="16" /> 清除
                </button>
              </div>
            </div>

            <div v-if="settings.sherpa_onnx_ref_audio_path" class="ref-text-group">
              <label class="config-label">参考文本（选择音频后必填）</label>
              <input
                v-model="settings.sherpa_onnx_ref_text"
                type="text"
                class="config-input"
                placeholder="与参考音频对应的文本，零样本克隆需与音频同时提供"
              />
            </div>
          </section>

          <!-- 测试合成 -->
          <section class="config-section test-section">
            <h4 class="section-title">
              <Play :size="16" />
              试听测试
            </h4>
            <div class="test-row">
              <input
                v-model="testText"
                type="text"
                class="config-input test-input"
                placeholder="输入测试文本"
              />
              <button
                @click="testVoice"
                class="btn-primary"
                :disabled="testing || !settings.sherpa_onnx_model_name"
              >
                <Loader2 v-if="testing" :size="16" class="spin" />
                <MicVocal v-else :size="16" />
                {{ testing ? "合成中..." : "试听" }}
              </button>
            </div>
            <p v-if="!settings.sherpa_onnx_model_name" class="test-hint">请先选择一个模型</p>
          </section>
        </div>

        <div class="modal-footer">
          <button @click="closeModal" class="btn-secondary">
            {{ $t("settings.shared.cancel") }}
          </button>
          <button @click="saveAndClose" class="btn-primary">
            <Check :size="16" /> {{ $t("settings.shared.save") }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import {
  X,
  Cpu,
  Package,
  Settings,
  SlidersHorizontal,
  Wand2,
  Mic,
  Play,
  Check,
  FolderOpen,
  Download,
  Trash2,
  Loader2,
  MicVocal,
  AlertTriangle,
  ShieldCheck,
} from "lucide-vue-next";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  listSherpaModels,
  downloadSherpaModel,
  deleteSherpaModel,
  onSherpaDownloadProgress,
  checkSherpaStoragePermission,
  requestSherpaStoragePermission,
  type SherpaOnnxModelRecord,
} from "@/api/services/tts/tts-local";
import { isAndroid } from "@/utils/platform";
import { playLocalAudio } from "@/utils/mediaUrl";

export interface SherpaOnnxSettings {
  sherpa_onnx_model_name: string;
  sherpa_onnx_model_path: string;
  sherpa_onnx_model_type: string;
  sherpa_onnx_lang: string;
  sherpa_onnx_voice: string;
  sherpa_onnx_use_gpu: boolean;
  sherpa_onnx_speed: number;
  sherpa_onnx_ref_audio_path: string;
  sherpa_onnx_ref_text: string;
}

const props = defineProps<{
  characterId: number;
  initialSettings: SherpaOnnxSettings;
}>();

const emit = defineEmits<{
  close: [];
  save: [settings: SherpaOnnxSettings];
}>();

const settings = ref<SherpaOnnxSettings>({
  sherpa_onnx_model_name: "",
  sherpa_onnx_model_path: "",
  sherpa_onnx_model_type: "vits",
  sherpa_onnx_lang: "zh",
  sherpa_onnx_voice: "female",
  sherpa_onnx_use_gpu: false,
  sherpa_onnx_speed: 1.0,
  sherpa_onnx_ref_audio_path: "",
  sherpa_onnx_ref_text: "",
});

const availableModels = ref<SherpaOnnxModelRecord[]>([]);
const downloading = ref<string | null>(null);
const deleting = ref<string | null>(null);
const progressByModel = ref<Record<string, number>>({});
const testing = ref(false);
const testText = ref("你好，这是一段测试语音。");
const storageGranted = ref(true);
const modelRoot = ref("");

let progressUnlisten: (() => void) | null = null;

const refAudioName = computed(() => {
  const p = settings.value.sherpa_onnx_ref_audio_path;
  if (!p) return "";
  return p.split(/[/\\]/).pop() || p;
});

onMounted(() => {
  if (props.initialSettings) {
    settings.value = { ...settings.value, ...props.initialSettings };
  }
  loadModels();
  void refreshStorageStatus();
  progressUnlisten = onSherpaDownloadProgress((progress) => {
    progressByModel.value = {
      ...progressByModel.value,
      [progress.asset_id]: progress.percent,
    };
  });
});

onUnmounted(() => {
  progressUnlisten?.();
  progressUnlisten = null;
});

const loadModels = async () => {
  try {
    availableModels.value = await listSherpaModels();
  } catch (error) {
    console.error("加载模型列表失败:", error);
    availableModels.value = [];
  }
};

/** 查询模型目录与存储权限（桌面端 granted 恒为 true）。 */
const refreshStorageStatus = async () => {
  try {
    const status = await checkSherpaStoragePermission();
    storageGranted.value = status.granted;
    modelRoot.value = status.model_root;
  } catch (error) {
    console.error("检查 Sherpa 存储权限失败:", error);
    storageGranted.value = isAndroid() ? false : true;
  }
};

/** 请求存储权限（Android 会弹出对话框 / 跳转系统设置页）。 */
const requestStoragePermission = async () => {
  try {
    const status = await requestSherpaStoragePermission();
    storageGranted.value = status.granted;
  } catch (error) {
    console.error("请求 Sherpa 存储权限失败:", error);
  }
};

const selectModel = (model: SherpaOnnxModelRecord) => {
  // 未安装的模型无本地文件，选中后试听/合成都只会失败；需先下载。
  if (!model.installed) return;
  settings.value.sherpa_onnx_model_name = model.id;
  settings.value.sherpa_onnx_model_path = model.path;
  settings.value.sherpa_onnx_lang = model.language;
  settings.value.sherpa_onnx_model_type = model.model_type;
  settings.value.sherpa_onnx_voice = model.voice;
};

const downloadModel = async (modelId: string) => {
  downloading.value = modelId;
  const next = { ...progressByModel.value };
  delete next[modelId];
  progressByModel.value = next;
  try {
    await downloadSherpaModel(modelId);
    progressByModel.value = { ...progressByModel.value, [modelId]: 100 };
    await loadModels();
  } catch (error) {
    console.error("下载模型失败:", error);
  } finally {
    downloading.value = null;
  }
};

const deleteModel = async (modelId: string) => {
  deleting.value = modelId;
  try {
    await deleteSherpaModel(modelId);
    if (settings.value.sherpa_onnx_model_name === modelId) {
      settings.value.sherpa_onnx_model_name = "";
      settings.value.sherpa_onnx_model_path = "";
    }
    await loadModels();
  } catch (error) {
    console.error("删除模型失败:", error);
  } finally {
    deleting.value = null;
  }
};

const openModelManager = async () => {
  try {
    await invoke("open_sherpa_onnx_model_manager");
  } catch (error) {
    console.error("打开模型管理器失败:", error);
  }
};

const pickRefAudio = async () => {
  try {
    const file = await open({
      multiple: false,
      filters: [{ name: "Audio", extensions: ["wav"] }],
    });
    if (file) {
      settings.value.sherpa_onnx_ref_audio_path = file as string;
    }
  } catch (error) {
    console.error("选择参考音频失败:", error);
  }
};

const clearRefAudio = () => {
  settings.value.sherpa_onnx_ref_audio_path = "";
  settings.value.sherpa_onnx_ref_text = "";
};

const testVoice = async () => {
  if (!testText.value.trim() || !settings.value.sherpa_onnx_model_name) return;
  testing.value = true;
  try {
    const result = await invoke<{ success: boolean; audio_path: string } | null>(
      "test_sherpa_onnx_voice",
      {
        text: testText.value,
        settings: settings.value,
      },
    );
    const audioPath = result?.audio_path;
    if (audioPath) {
      await playLocalAudio(audioPath);
    } else {
      console.warn("语音测试未返回音频路径");
    }
  } catch (error) {
    console.error("语音测试失败:", error);
  } finally {
    testing.value = false;
  }
};

const saveAndClose = () => {
  emit("save", settings.value);
  closeModal();
};

const closeModal = () => {
  emit("close");
};

const formatSize = (bytes: number): string => {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
};
</script>

<style scoped>
.sherpa-modal {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 1100;
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-overlay {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(4px);
}

.modal-content {
  position: relative;
  background: linear-gradient(145deg, rgba(20, 20, 35, 0.98), rgba(15, 15, 28, 0.98));
  border: 1px solid rgba(139, 92, 246, 0.2);
  border-radius: 16px;
  width: 92%;
  max-width: 520px;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  box-shadow:
    0 25px 60px rgba(0, 0, 0, 0.5),
    0 0 40px rgba(139, 92, 246, 0.08);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 18px 20px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.header-icon {
  color: rgba(139, 92, 246, 0.9);
}

.modal-header h3 {
  color: white;
  font-size: 17px;
  font-weight: 600;
  margin: 0;
}

.close-btn {
  background: none;
  border: none;
  color: rgba(255, 255, 255, 0.5);
  cursor: pointer;
  padding: 6px;
  border-radius: 8px;
  transition: all 0.15s;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.08);
  color: white;
}

.modal-body {
  padding: 16px 20px;
  overflow-y: auto;
  flex: 1;
}

.modal-body::-webkit-scrollbar {
  width: 4px;
}

.modal-body::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.15);
  border-radius: 2px;
}

/* Sections */
.config-section {
  margin-bottom: 20px;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  color: rgba(255, 255, 255, 0.85);
  font-size: 13px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin: 0 0 12px 0;
}

.section-desc {
  color: rgba(255, 255, 255, 0.45);
  font-size: 12px;
  margin: -8px 0 12px 0;
}

/* Model list */
.model-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 10px;
  max-height: 180px;
  overflow-y: auto;
}

.model-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.15s;
}

.model-item:hover {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(139, 92, 246, 0.25);
}

.model-item.selected {
  background: rgba(139, 92, 246, 0.12);
  border-color: rgba(139, 92, 246, 0.4);
}

.model-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-width: 0;
}

.model-name {
  color: white;
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.model-meta {
  color: rgba(255, 255, 255, 0.4);
  font-size: 11px;
}

.model-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
  margin-left: 8px;
}

.model-progress {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
  width: 100%;
}

.progress-track {
  flex: 1;
  height: 4px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.1);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, rgba(139, 92, 246, 0.6), rgba(139, 92, 246, 0.9));
  border-radius: 2px;
  transition: width 0.15s ease;
}

.progress-text {
  color: rgba(139, 92, 246, 0.9);
  font-size: 11px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  min-width: 34px;
  text-align: right;
}

.no-models {
  color: rgba(255, 255, 255, 0.35);
  font-size: 12px;
  text-align: center;
  padding: 16px;
}

/* 模型存放目录展示 */
.model-dir-info {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  margin-bottom: 8px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  min-width: 0;
}

.model-dir-icon {
  color: rgba(255, 255, 255, 0.4);
  flex-shrink: 0;
}

.model-dir-label {
  color: rgba(255, 255, 255, 0.55);
  font-size: 11px;
  flex-shrink: 0;
}

.model-dir-path {
  color: rgba(139, 92, 246, 0.9);
  font-size: 11px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
  min-width: 0;
}

/* Android 存储权限未授权警告 */
.perm-warning {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 10px 12px;
  margin-bottom: 8px;
  background: rgba(234, 179, 8, 0.08);
  border: 1px solid rgba(234, 179, 8, 0.35);
  border-radius: 8px;
}

.perm-warning-icon {
  color: rgba(234, 179, 8, 0.9);
  flex-shrink: 0;
  margin-top: 2px;
}

.perm-warning-text {
  flex: 1;
  min-width: 0;
}

.perm-warning-text p {
  color: rgba(255, 255, 255, 0.85);
  font-size: 12px;
  line-height: 1.5;
  margin: 0 0 8px 0;
}

/* Config grid */
.config-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.config-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.config-label {
  color: rgba(255, 255, 255, 0.6);
  font-size: 12px;
  font-weight: 500;
}

.config-select,
.config-input {
  width: 100%;
  padding: 8px 10px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  color: white;
  font-size: 13px;
  transition: border-color 0.15s;
}

.config-select:focus,
.config-input:focus {
  outline: none;
  border-color: rgba(139, 92, 246, 0.5);
}

.config-input:read-only {
  opacity: 0.6;
  cursor: not-allowed;
}

/* Toggle */
.toggle-wrapper {
  cursor: pointer;
  display: inline-flex;
}

.toggle-input {
  display: none;
}

.toggle-track {
  position: relative;
  width: 40px;
  height: 22px;
  background: rgba(255, 255, 255, 0.15);
  border-radius: 11px;
  transition: background 0.2s;
}

.toggle-thumb {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 16px;
  height: 16px;
  background: white;
  border-radius: 50%;
  transition: transform 0.2s;
}

.toggle-input:checked + .toggle-track {
  background: rgba(139, 92, 246, 0.6);
}

.toggle-input:checked + .toggle-track .toggle-thumb {
  transform: translateX(18px);
}

/* Parameters */
.param-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.param-item {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.param-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.param-label {
  color: rgba(255, 255, 255, 0.6);
  font-size: 12px;
}

.param-value {
  color: rgba(139, 92, 246, 0.9);
  font-size: 12px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.param-slider {
  width: 100%;
  height: 4px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 2px;
  outline: none;
  -webkit-appearance: none;
}

.param-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 14px;
  height: 14px;
  background: rgba(139, 92, 246, 0.9);
  border-radius: 50%;
  cursor: pointer;
  transition: transform 0.1s;
}

.param-slider::-webkit-slider-thumb:hover {
  transform: scale(1.2);
}

/* Zero-shot section */
.zero-shot-section {
  background: rgba(139, 92, 246, 0.04);
  border: 1px solid rgba(139, 92, 246, 0.1);
  border-radius: 12px;
  padding: 14px;
}

.ref-audio-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  margin-bottom: 10px;
  transition: all 0.15s;
}

.ref-audio-card.active {
  border-color: rgba(34, 197, 94, 0.3);
  background: rgba(34, 197, 94, 0.04);
}

.ref-audio-info {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1;
}

.ref-audio-icon {
  color: rgba(255, 255, 255, 0.3);
  flex-shrink: 0;
}

.ref-audio-card.active .ref-audio-icon {
  color: rgba(34, 197, 94, 0.7);
}

.ref-audio-detail {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.ref-audio-name {
  color: white;
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ref-audio-status {
  color: rgba(255, 255, 255, 0.4);
  font-size: 11px;
}

.ref-audio-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
  margin-left: 10px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.ref-text-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* Test section */
.test-section {
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 12px;
  padding: 14px;
}

.test-row {
  display: flex;
  gap: 8px;
}

.test-row .btn-primary {
  flex-shrink: 0;
}

.test-input {
  flex: 1;
  min-width: 0;
}

.test-hint {
  color: rgba(255, 255, 255, 0.4);
  font-size: 11px;
  margin-top: 6px;
}

/* Buttons */
.btn-primary,
.btn-secondary {
  padding: 8px 14px;
  border: none;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
}

.btn-primary {
  background: rgba(139, 92, 246, 0.8);
  color: white;
}

.btn-primary:hover:not(:disabled) {
  background: rgba(139, 92, 246, 1);
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-secondary {
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.8);
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.btn-secondary:hover {
  background: rgba(255, 255, 255, 0.1);
}

.btn-danger {
  color: rgba(239, 68, 68, 0.9);
  border-color: rgba(239, 68, 68, 0.3);
}

.btn-danger:hover {
  background: rgba(239, 68, 68, 0.15);
}

.btn-sm {
  padding: 6px 12px;
  font-size: 12px;
}

.btn-block {
  width: 100%;
  justify-content: center;
}

.btn-icon {
  background: none;
  border: 1px solid rgba(255, 255, 255, 0.12);
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  padding: 5px;
  border-radius: 6px;
  transition: all 0.15s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.btn-icon:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.btn-download {
  border-color: rgba(34, 197, 94, 0.3);
}

.btn-download:hover:not(:disabled) {
  background: rgba(34, 197, 94, 0.15);
  border-color: rgba(34, 197, 94, 0.5);
}

.btn-delete {
  border-color: rgba(239, 68, 68, 0.3);
}

.btn-delete:hover:not(:disabled) {
  background: rgba(239, 68, 68, 0.15);
  border-color: rgba(239, 68, 68, 0.5);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

/* Footer */
.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 14px 20px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

@media (max-width: 520px) {
  .config-grid {
    grid-template-columns: 1fr;
  }
  .param-grid {
    grid-template-columns: 1fr;
  }
  .ref-audio-card {
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }
  .ref-audio-actions {
    margin-left: 0;
    width: 100%;
    justify-content: flex-start;
  }
  .ref-audio-actions .btn-secondary {
    flex: 1;
    justify-content: center;
  }
  .test-row {
    flex-wrap: wrap;
  }
  .test-input {
    flex: 1 1 100%;
  }
  .test-row .btn-primary {
    width: 100%;
    justify-content: center;
  }
}

@media (max-height: 700px) and (max-width: 520px) {
  .modal-content {
    max-height: 92dvh;
  }
  .model-list {
    max-height: 140px;
  }
  .modal-header {
    padding: 14px 16px;
  }
  .modal-body {
    padding: 12px 16px;
  }
  .modal-footer {
    padding: 10px 16px;
  }
  .config-section {
    margin-bottom: 14px;
  }
}
</style>

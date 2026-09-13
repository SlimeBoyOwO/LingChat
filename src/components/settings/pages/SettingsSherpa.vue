<template>
  <MenuPage>
    <MenuItem :title="t('settings.sherpa.title')" size="large">
      <template #header>
        <Cpu :size="20" class="text-violet-300" />
      </template>

      <div class="flex min-h-0 flex-col gap-6">
        <!-- 模型列表 -->
        <section class="config-section">
          <h4 class="section-title">
            <Package :size="16" />
            {{ t("settings.sherpa.models") }}
          </h4>
          <div class="model-list">
            <div
              v-for="model in availableModels"
              :key="model.id"
              class="model-item"
              :class="{ selected: selectedModelId === model.id }"
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
                  class="btn-icon btn-download"
                  :disabled="downloading === model.id"
                  @click.stop="downloadModel(model.id)"
                >
                  <Loader2 v-if="downloading === model.id" :size="14" class="spin" />
                  <Download v-else :size="14" />
                </button>
                <button
                  v-else
                  class="btn-icon btn-delete"
                  :disabled="deleting === model.id"
                  @click.stop="deleteModel(model.id)"
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
                <span class="progress-text">{{ Math.round(progressByModel[model.id] ?? 0) }}%</span>
              </div>
            </div>
            <p v-if="availableModels.length === 0" class="no-models">
              {{ t("settings.sherpa.noModels") }}
            </p>
          </div>
          <button class="btn-secondary btn-block" @click="openModelManager">
            <FolderOpen :size="16" /> {{ t("settings.sherpa.manageModels") }}
          </button>
        </section>

        <!-- 模型配置 -->
        <section class="config-section">
          <h4 class="section-title">
            <Settings :size="16" />
            {{ t("settings.sherpa.modelConfig") }}
          </h4>
          <div class="config-grid">
            <div class="config-item">
              <label class="config-label">{{ t("settings.sherpa.modelType") }}</label>
              <select v-model="config.model_type" class="config-select">
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
              <label class="config-label">{{ t("settings.sherpa.language") }}</label>
              <select v-model="config.lang" class="config-select">
                <option value="zh">中文</option>
                <option value="en">English</option>
                <option value="ja">日本語</option>
                <option value="ko">한국어</option>
              </select>
            </div>
            <div class="config-item">
              <label class="config-label">{{ t("settings.sherpa.voice") }}</label>
              <select v-model="config.voice" class="config-select">
                <option value="female">女声</option>
                <option value="male">男声</option>
                <option value="child">童声</option>
                <option value="elderly">老人</option>
              </select>
            </div>
            <div class="config-item">
              <label class="config-label">{{ t("settings.sherpa.useGpu") }}</label>
              <label class="toggle-wrapper">
                <input v-model="config.use_gpu" type="checkbox" class="toggle-input" />
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
            {{ t("settings.sherpa.parameters") }}
          </h4>
          <div class="param-grid">
            <div class="param-item">
              <div class="param-header">
                <label class="param-label">{{ t("settings.sherpa.speed") }}</label>
                <span class="param-value">{{ config.speed.toFixed(1) }}</span>
              </div>
              <input
                v-model.number="config.speed"
                type="range"
                min="0.5"
                max="2.0"
                step="0.1"
                class="param-slider"
              />
            </div>
          </div>
        </section>

        <!-- 零样本声音克隆 -->
        <section class="config-section zero-shot-section">
          <h4 class="section-title">
            <Wand2 :size="16" />
            {{ t("settings.sherpa.zeroShot") }}
          </h4>
          <p class="section-desc">{{ t("settings.sherpa.zeroShotDesc") }}</p>

          <div class="ref-audio-card" :class="{ active: config.ref_audio_path }">
            <div class="ref-audio-info">
              <Mic :size="24" class="ref-audio-icon" />
              <div v-if="config.ref_audio_path" class="ref-audio-detail">
                <span class="ref-audio-name">{{ refAudioName }}</span>
                <span class="ref-audio-status">{{ t("settings.sherpa.refLoaded") }}</span>
              </div>
              <div v-else class="ref-audio-detail">
                <span class="ref-audio-name">{{ t("settings.sherpa.refNone") }}</span>
                <span class="ref-audio-status">仅支持 WAV 格式</span>
              </div>
            </div>
            <div class="ref-audio-actions">
              <button class="btn-secondary" @click="pickRefAudio">
                <FolderOpen :size="16" />
                {{
                  config.ref_audio_path
                    ? t("settings.sherpa.refChange")
                    : t("settings.sherpa.refPick")
                }}
              </button>
              <button
                v-if="config.ref_audio_path"
                class="btn-secondary btn-danger"
                @click="
                  config.ref_audio_path = '';
                  config.ref_text = '';
                "
              >
                <X :size="16" /> {{ t("settings.sherpa.refClear") }}
              </button>
            </div>
          </div>

          <div v-if="config.ref_audio_path" class="ref-text-group">
            <label class="config-label">{{ t("settings.sherpa.refText") }}</label>
            <input
              v-model="config.ref_text"
              type="text"
              class="config-input"
              :placeholder="t('settings.sherpa.refTextPlaceholder')"
            />
          </div>
        </section>

        <!-- 试听测试 -->
        <section class="config-section test-section">
          <h4 class="section-title">
            <Play :size="16" />
            {{ t("settings.sherpa.test") }}
          </h4>
          <div class="test-row">
            <input
              v-model="testText"
              type="text"
              class="config-input test-input"
              :placeholder="t('settings.sherpa.testPlaceholder')"
            />
            <button
              class="btn-primary"
              :disabled="testing || playlist !== null || !selectedModelId"
              @click="testVoice"
            >
              <Loader2 v-if="testing" :size="16" class="spin" />
              <Play v-else :size="16" />
              {{ testing ? t("settings.sherpa.synthesizing") : t("settings.sherpa.listen") }}
            </button>
            <button
              v-if="playlist !== null"
              class="btn-secondary"
              title="停止播放"
              @click="stopPreview"
            >
              <Square :size="16" />
            </button>
          </div>
          <p v-if="!selectedModelId" class="test-hint">{{ t("settings.sherpa.noModelHint") }}</p>
        </section>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
  import { ref, computed, onMounted, onUnmounted } from "vue";
  import { useI18n } from "vue-i18n";
  import {
    Cpu,
    Package,
    Settings,
    SlidersHorizontal,
    Wand2,
    Mic,
    Play,
    Square,
    FolderOpen,
    Download,
    Trash2,
    Loader2,
    X,
  } from "lucide-vue-next";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { MenuItem, MenuPage } from "../../ui";
  import {
    listSherpaModels,
    downloadSherpaModel,
    deleteSherpaModel,
    onSherpaDownloadProgress,
    type SherpaOnnxModelRecord,
  } from "@/api/services/tts/tts-local";
  import { playLocalAudio, stopCurrentAudio } from "@/utils/mediaUrl";

  const { t } = useI18n();

  const availableModels = ref<SherpaOnnxModelRecord[]>([]);
  const selectedModelId = ref("");
  const downloading = ref<string | null>(null);
  const deleting = ref<string | null>(null);
  const progressByModel = ref<Record<string, number>>({});
  const testing = ref(false);
  const playlist = ref<HTMLAudioElement | null>(null);
  const testText = ref("你好，这是一段测试语音。");

  let progressUnlisten: (() => void) | null = null;

  const config = ref({
    model_type: "vits",
    lang: "zh",
    voice: "female",
    use_gpu: false,
    speed: 1.0,
    ref_audio_path: "",
    ref_text: "",
  });

  const refAudioName = computed(() => {
    const p = config.value.ref_audio_path;
    if (!p) return "";
    return p.split(/[/\\]/).pop() || p;
  });

  onMounted(() => {
    loadModels();
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

  async function loadModels() {
    try {
      availableModels.value = await listSherpaModels();
    } catch (error) {
      console.error("加载模型列表失败:", error);
      availableModels.value = [];
    }
  }

  function selectModel(model: SherpaOnnxModelRecord) {
    // 未安装的模型无本地文件，选中后试听只会失败；需先下载。
    if (!model.installed) return;
    selectedModelId.value = model.id;
    config.value.model_type = model.model_type;
    config.value.lang = model.language;
    config.value.voice = model.voice;
  }

  async function downloadModel(modelId: string) {
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
  }

  async function deleteModel(modelId: string) {
    deleting.value = modelId;
    try {
      await deleteSherpaModel(modelId);
      if (selectedModelId.value === modelId) {
        selectedModelId.value = "";
      }
      await loadModels();
    } catch (error) {
      console.error("删除模型失败:", error);
    } finally {
      deleting.value = null;
    }
  }

  async function openModelManager() {
    try {
      await invoke("open_sherpa_onnx_model_manager");
    } catch (error) {
      console.error("打开模型管理器失败:", error);
    }
  }

  async function pickRefAudio() {
    try {
      const file = await open({
        multiple: false,
        filters: [{ name: "Audio", extensions: ["wav"] }],
      });
      if (file) {
        config.value.ref_audio_path = file as string;
      }
    } catch (error) {
      console.error("选择参考音频失败:", error);
    }
  }

  async function testVoice() {
    if (!testText.value.trim() || !selectedModelId.value) return;
    testing.value = true;
    try {
      const result = await invoke<{ success: boolean; audio_path: string } | null>(
        "test_sherpa_onnx_voice",
        {
          text: testText.value,
          settings: {
            sherpa_onnx_model_name: selectedModelId.value,
            sherpa_onnx_model_path: "",
            sherpa_onnx_model_type: config.value.model_type,
            sherpa_onnx_lang: config.value.lang,
            sherpa_onnx_voice: config.value.voice,
            sherpa_onnx_use_gpu: config.value.use_gpu,
            sherpa_onnx_speed: config.value.speed,
            sherpa_onnx_ref_audio_path: config.value.ref_audio_path,
            sherpa_onnx_ref_text: config.value.ref_text,
          },
        }
      );
      if (result?.audio_path) {
        playlist.value = await playLocalAudio(result.audio_path);
        playlist.value.onended = () => {
          playlist.value = null;
        };
        // 加载/解码失败时同样复位，否则试听按钮会一直处于禁用状态。
        playlist.value.onerror = () => {
          playlist.value = null;
        };
      } else {
        console.warn("语音测试未返回音频路径");
      }
    } catch (error) {
      console.error("语音测试失败:", error);
    } finally {
      testing.value = false;
    }
  }

  function stopPreview() {
    playlist.value = null;
    stopCurrentAudio();
  }

  const formatSize = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  };
</script>

<style scoped>
  .config-section {
    margin-bottom: 4px;
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

  .model-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 10px;
    max-height: 240px;
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
  }

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
  }

  .ref-text-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

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

  .test-input {
    flex: 1;
  }

  .test-hint {
    color: rgba(255, 255, 255, 0.4);
    font-size: 11px;
    margin-top: 6px;
  }

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

  @media (max-width: 520px) {
    .config-grid,
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
    }
  }
</style>

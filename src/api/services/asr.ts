import { invoke } from "@tauri-apps/api/core";

export type AsrSource = "button" | "auto";
export type SendMode = "fill_only" | "auto_send";
export type AsrPhase = "idle" | "recording" | "recognizing";

export interface AsrResult {
  text: string;
  language?: string;
  confidence?: number;
  provider_id: string;
}

export interface ProviderConfig {
  api_key: string;
  endpoint: string;
  model: string;
  extra?: Record<string, string>;
}

export interface ModelInfo {
  id: string;
  display_name: string;
  supports_streaming: boolean;
  is_default: boolean;
  /** 协议端点预设（选中该模型时同步填入 endpoint；None 用当前配置） */
  endpoint?: string | null;
}

export interface AsrSettings {
  active_provider: string;
  auto_listen: boolean;
  send_mode: SendMode;
  stream_enabled: boolean;
  voice_input_enabled: boolean;
  /** VAD 静音计时（毫秒）：停止说话后静音该时长才结束一轮录音（默认 800） */
  vad_silence_ms: number;
  /** 能量监测启动缓冲期（毫秒）：TTS 播完恢复监听后该时长内不触发录音（默认 100，0=无缓冲） */
  energy_warmup_ms: number;
  provider_configs: Record<string, ProviderConfig>;
}

/** 与后端 `provider.rs` 的 `ConfigFieldKind`（snake_case 字符串）严格对齐 */
export type ConfigFieldKind = "text" | "password" | "number" | "boolean";

export interface AsrConfigField {
  key: string;
  label: string;
  kind: ConfigFieldKind;
  required: boolean;
  default_value?: string;
  placeholder?: string;
  hint?: string;
}

export interface ProviderInfo {
  id: string;
  display_name: string;
  /** 简短描述（设置页服务商选择旁展示） */
  description?: string;
  config_fields: AsrConfigField[];
  supports_streaming: boolean;
}

export interface VadEvent {
  type: "speech_started" | "silence_started" | "turn_candidate" | "turn_sealed";
  silence_ms?: number;
}

export const asrStartListening = (source: AsrSource) =>
  invoke<void>("asr_start_listening", { source });

export const asrStopListening = (source: AsrSource) =>
  invoke<void>("asr_stop_listening", { source });

export const asrVadProcessChunk = (pcm: number[]) => invoke<void>("asr_vad_process_chunk", { pcm });

export const asrRecognizeWav = (params: {
  providerId: string;
  wavBytes: number[];
  languageHint?: string | null;
}) =>
  invoke<AsrResult>("asr_recognize_wav", {
    providerId: params.providerId,
    wavBytes: params.wavBytes,
    languageHint: params.languageHint ?? null,
  });

/** 结果流式识别（llama-asr SSE）：整段 WAV 上传 → partial 事件 → final。
 *  与 WS 会话流式（asr_start_streaming 系列）独立。 */
export const asrRecognizeWavStream = (params: { providerId: string; wavBytes: number[] }) =>
  invoke<AsrResult>("asr_recognize_wav_stream", {
    providerId: params.providerId,
    wavBytes: params.wavBytes,
  });

export const asrCancel = () => invoke<void>("asr_cancel");

export const asrListProviders = () => invoke<ProviderInfo[]>("asr_list_providers");

export const asrListModels = (providerId: string) =>
  invoke<ModelInfo[]>("asr_list_models", { providerId });

export const asrGetSettings = () => invoke<AsrSettings>("asr_get_settings");

/** ASR 运行时状态（设置页状态面板） */
export interface AsrStatus {
  /** VAD 模型是否加载成功（session 存在 = init_asr 完成） */
  vad_loaded: boolean;
}

export const asrGetStatus = () => invoke<AsrStatus>("asr_get_status");

export const asrSetSettings = (settings: AsrSettings) =>
  invoke<void>("asr_set_settings", { settings });

export const asrTestProvider = (providerId: string) =>
  invoke<void>("asr_test_provider", { providerId });

export const asrStartStreaming = (params: { providerId: string; languageHint?: string | null }) =>
  invoke<void>("asr_start_streaming", {
    providerId: params.providerId,
    languageHint: params.languageHint ?? null,
  });

export const asrStreamAudioChunk = (pcm: number[]) =>
  invoke<void>("asr_stream_audio_chunk", { pcm });

export const asrStopStreaming = () => invoke<AsrResult>("asr_stop_streaming");

/** 丢弃流式会话（异常路径清理用；不影响非流式在飞识别） */
export const asrCancelStreaming = () => invoke<void>("asr_cancel_streaming");

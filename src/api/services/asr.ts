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
  /**
   * 按地域分存的 API Key，键为地域 id（"cn-beijing" / "ap-southeast-1"）。
   *
   * 各地域的 Key 互相独立、不能混用；设置页只显示当前地域的那一个框，
   * 读写都落在这里（`api_keys[当前地域]`）。
   */
  api_keys?: Record<string, string>;
  /** 历史字段：分地域存储之前的唯一 Key。后端仍作为兜底读取，设置页不再写入 */
  api_key: string;
  /** 非实时（同步）端点 */
  endpoint: string;
  /** 实时（流式）WebSocket 端点；与 endpoint 独立（协议不同，用户可能只覆盖其一） */
  ws_endpoint?: string;
  model: string;
  /** DashScope 地域 id（"cn-beijing" / "ap-southeast-1"）；空/未知 = 默认地域 */
  region?: string;
  // 热词不在这里：热词逐角色（存数据库），按调用经识别命令的 `hotwords`
  // 参数传入，见下方 HotwordInput。
}

/** 模型对应的端点类型：选中该模型时把对应端点字段填成当前地域的默认值 */
export type EndpointKind = "http" | "ws";

export interface ModelInfo {
  id: string;
  display_name: string;
  supports_streaming: boolean;
  is_default: boolean;
  /**
   * 端点预设类型。只给类型不给完整 URL —— 端点是地域相关的，
   * 实际地址从 activeProviderInfo.regions 按当前地域取（避免两处真相）。
   * null = 不干预端点（本地 llama-asr 的地址与模型无关）。
   */
  endpoint_kind?: EndpointKind | null;
}

/** 与后端 region.rs 的 AsrRegionInfo 对齐：某地域的端点默认值 */
export interface AsrRegionInfo {
  id: string;
  label: string;
  http_endpoint: string;
  ws_endpoint: string;
}

/** 一次识别调用的热词：纯词（默认权重）或带权重对象 */
export type HotwordInput = string | { text: string; weight?: number };

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
  /** 是否输出逐帧 VAD 能量检测日志（frame/prob/len，默认关，排查识别不触发时打开） */
  vad_debug_log: boolean;
  /** 语音输入快捷键（ShortcutBinding JSON 字符串，默认 {"key":"f8"}） */
  ptt_key: string;
  /** 失去焦点快捷键可用（全局快捷键）：窗口不在前台时快捷键仍可用（默认关） */
  ptt_global: boolean;
  provider_configs: Record<string, ProviderConfig>;
}

/** 与后端 `provider.rs` 的 `ConfigFieldKind`（snake_case 字符串）严格对齐 */
export type ConfigFieldKind =
  | "text"
  | "password"
  | "number"
  | "boolean"
  | "select"
  /** 密码框，但值按地域分开存在 `provider_configs[id].api_keys[当前地域]` */
  | "password_map";

/** select 字段的一个选项 */
export interface AsrConfigFieldOption {
  value: string;
  label: string;
}

export interface AsrConfigField {
  key: string;
  label: string;
  kind: ConfigFieldKind;
  required: boolean;
  default_value?: string;
  placeholder?: string;
  hint?: string;
  /** 仅 kind === "select" 时有值 */
  options?: AsrConfigFieldOption[];
}

export interface ProviderInfo {
  id: string;
  display_name: string;
  /** 简短描述（设置页服务商选择旁展示） */
  description?: string;
  config_fields: AsrConfigField[];
  supports_streaming: boolean;
  /**
   * 可选地域列表（含各地域端点默认值）。空/缺省 = 该 provider 无地域概念
   * （如本地 llama-asr），不渲染地域下拉、切地域联动也不生效。
   */
  regions?: AsrRegionInfo[];
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

/** `hotwords` 是热词的**唯一入口**（ASR 设置页已无热词设置）：热词逐角色、
 *  存数据库，接入时在这里传入即可，provider 侧无需改动。目前一律省略。 */
export const asrRecognizeWav = (params: {
  providerId: string;
  wavBytes: number[];
  languageHint?: string | null;
  hotwords?: HotwordInput[] | null;
}) =>
  invoke<AsrResult>("asr_recognize_wav", {
    providerId: params.providerId,
    wavBytes: params.wavBytes,
    languageHint: params.languageHint ?? null,
    hotwords: params.hotwords ?? null,
  });

/** 结果流式识别（llama-asr SSE）：整段 WAV 上传 → partial 事件 → final。
 *  与 WS 会话流式（asr_start_streaming 系列）独立。 */
export const asrRecognizeWavStream = (params: {
  providerId: string;
  wavBytes: number[];
  hotwords?: HotwordInput[] | null;
}) =>
  invoke<AsrResult>("asr_recognize_wav_stream", {
    providerId: params.providerId,
    wavBytes: params.wavBytes,
    hotwords: params.hotwords ?? null,
  });

export const asrCancel = () => invoke<void>("asr_cancel");

export const asrListProviders = () => invoke<ProviderInfo[]>("asr_list_providers");

/**
 * 拉取某 provider 的模型清单。
 *
 * `region` 传表单里**当前选中**的地域：模型清单是按地域过滤的，而保存设置
 * 有 debounce（500ms），不传的话后端会按上一次落盘的地域返回——切地域后列表
 * 要等下次打开设置页才更新（表现为"少/多一个模型"）。缺省/空 = 用落盘的配置。
 */
export const asrListModels = (providerId: string, region?: string) =>
  invoke<ModelInfo[]>("asr_list_models", { providerId, region: region ?? null });

export const asrGetSettings = () => invoke<AsrSettings>("asr_get_settings");

/** ASR 运行时状态（设置页状态面板） */
export interface AsrStatus {
  /** VAD 模型是否加载成功（session 存在 = init_asr 完成） */
  vad_loaded: boolean;
  /** 全局快捷键注册是否健康（ptt_global=true 且实际已注册） */
  ptt_global_ok: boolean;
}

export const asrGetStatus = () => invoke<AsrStatus>("asr_get_status");

/** 全局快捷键注册状态（`asr:ptt-global-status` 事件载荷的判别字段）。
 *
 * 三态必须分清：`inactive`（开关关 / 不在聊天界面）是**正常状态**，不应提示；
 * 只有 `failed` 才是真失败。历史上后端用单个 `ok: boolean` 同时表达这两者，
 * 关闭开关时设置页会误报「全局快捷键注册失败：」（reason 为空）。 */
export type PttGlobalState = "registered" | "inactive" | "failed";

export interface PttGlobalStatus {
  state: PttGlobalState;
  /** 仅 `failed` 时非空：注册失败的原因。 */
  reason: string;
}

/** 全局快捷键界面门控（仅 /chat 与 /pet 激活）：离开界面注销释放键位
 *  （OS 级注册会拦截其它应用的同键输入），回界面按设置重注册。移动端 no-op。 */
export const asrPttGlobalSetActive = (active: boolean) =>
  invoke<void>("asr_ptt_global_set_active", { active });

export const asrSetSettings = (settings: AsrSettings) =>
  invoke<void>("asr_set_settings", { settings });

export const asrTestProvider = (providerId: string) =>
  invoke<void>("asr_test_provider", { providerId });

export const asrStartStreaming = (params: {
  providerId: string;
  languageHint?: string | null;
  hotwords?: HotwordInput[] | null;
}) =>
  invoke<void>("asr_start_streaming", {
    providerId: params.providerId,
    languageHint: params.languageHint ?? null,
    hotwords: params.hotwords ?? null,
  });

export const asrStreamAudioChunk = (pcm: number[]) =>
  invoke<void>("asr_stream_audio_chunk", { pcm });

export const asrStopStreaming = () => invoke<AsrResult>("asr_stop_streaming");

/** 丢弃流式会话（异常路径清理用；不影响非流式在飞识别） */
export const asrCancelStreaming = () => invoke<void>("asr_cancel_streaming");

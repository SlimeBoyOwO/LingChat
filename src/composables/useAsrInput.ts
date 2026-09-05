import { listen } from "@tauri-apps/api/event";
import { computed, ref, shallowRef, watch } from "vue";
import { useRoute, type RouteLocationNormalizedLoaded } from "vue-router";

import {
  asrCancel,
  asrCancelStreaming,
  asrRecognizeWav,
  asrRecognizeWavStream,
  asrStartListening,
  asrStartStreaming,
  asrStopListening,
  asrStopStreaming,
  asrStreamAudioChunk,
  asrVadProcessChunk,
  type AsrSource,
  type VadEvent,
} from "@/api/services/asr";
import { useGameStore } from "@/stores/modules/game";
import { useAsrStore } from "@/stores/modules/settings/asr";
import { useUIStore } from "@/stores/modules/ui/ui";
import { pcmToWavPcm16, trimSilencePcm } from "@/utils/asrAudio";
import { parseAsrError } from "@/utils/asrError";

/**
 * 统一 ASR 输入入口：两种触发源共用同一会话生命周期。
 *
 * 两种触发源：
 * - Button: GameDialog.vue / ChatInput.vue（桌宠）的 mic 按钮
 * - Auto: asrStore.settings.auto_listen=true 时由能量监测触发
 *
 * 窗口活跃门控：仅当 chatActive=true（/chat 或 /pet 路由 + 设置抽屉未开）时启用。
 * 失败降级：mic 不可用时 fail-open（不抛错到用户），退化为手动按钮 + 不录。
 *
 * ── 单例设计 ──────────────────────────────────────────────
 * 状态全部在模块级（非函数内）：App.vue 的初始化实例与 GameDialog /
 * ChatInput 的 mic 实例共享同一会话。若状态放在函数内，两实例各自持有
 * recorder/phase，mic 按钮看不到录音状态、互不感知。
 *
 * ── 采集链路（spec §3.1）─────────────────────────────────
 * 16kHz AudioContext + ScriptProcessor 直接拿 f32 PCM（不经过
 * MediaRecorder webm 编码），停止时合成 16k mono PCM16 WAV 送去识别。
 * auto 模式额外把每 512 samples（30ms）喂 asrVadProcessChunk，
 * 由后端 Silero VAD 做端点检测（turn_candidate → 一轮说话结束）。
 *
 * 队列设计说明：项目里没有专门的 useChatStore（聊天状态由 useGameStore.currentStatus
 * 体现：'input' = 空闲可输入，'thinking'/'responding'/'presenting' = 生成中）。
 * auto_send 由后端 generation_lock 排队，无需前端队列（queue 模式已移除）。
 */

// ── 模块级单例状态 ──────────────────────────────────────────
const phase = ref<"idle" | "recording" | "recognizing">("idle");
const activeSource = shallowRef<AsrSource | null>(null);

/** 本次录音累积的 f32 PCM（16kHz mono） */
let pcmBuffer: number[] = [];
/** 待喂 VAD 的积累块（凑满 512 samples = 30ms 才发） */
let vadPending: number[] = [];
let stream: MediaStream | null = null;
let audioCtx: AudioContext | null = null;
let processor: ScriptProcessorNode | null = null;
let energyMon: { ctx: AudioContext; raf: number; stream: MediaStream } | null = null;
/** auto 触发去重：能量触发后不再重复触发，直到本轮会话结束 */
let autoTriggered = false;
/** 移动端菜单展开状态（GameDialog 在 watch 中同步，§1.5 判定） */
let mobileMenuOpen = false;
/** 短暂显示锁：识别后填入 inputMessage 到自动 send 之间的窗口期，期间 auto 触发禁用（§1.10）。
 *  ref 化（非普通变量）：canStartMic 等 computed 依赖它，锁过期后能自动重算解锁。 */
const asrLockedUntil = ref(0);
/** auto_send 模式：识别完成后延迟发送的毫秒数（给用户看到结果的窗口，防乱序）。
 *  导出供 GameDialog / ChatInput 的 asr-send 监听复用（同一延迟语义）。 */
export const ASR_AUTO_SEND_DELAY_MS = 800;
/** 录音硬上限（samples）：1 分钟 @ 16kHz。达到后自动 stop()——
 *  防止按钮长按/异常会话无限录音（VAD 端 max_segment_frames 同为 60s，两处对齐；
 *  有界也顺带解决长时间录音时 pcmBuffer 的无限内存增长）。 */
const MAX_RECORD_SAMPLES = 60 * 16000;
/** 能量监测启动缓冲期兜底值（毫秒）：未加载设置时用 100ms。
 *  实际值来自 asrStore.settings.energy_warmup_ms（设置页可自定义，
 *  0 = 无缓冲）。voicePlaying 门控已保证 TTS 播放期间完全不监听，
 *  此缓冲期只兜底播放结束瞬间的残响尾巴。 */
const ENERGY_WARMUP_MS = 100;
/** 角色语音（TTS）播放中（GameRolesStage 桌面/桌宠通过 setVoicePlaying 同步）：
 *  外放 TTS 会被麦克风捕获 → RMS 触发 → VAD 判定为人声 → 误识别 AI 自己的话。
 *  播放期间 ASR 整体禁用（canStartAsr 门控 + handle drop），播完才恢复。
 *  ref 化：canStartMic 等 computed 依赖它，播完 setVoicePlaying(false) 自动解锁。 */
const voicePlaying = ref(false);
/** 输入框桥：GameDialog 注册，供 partial 实时写入 / 拼接基准读取 */
let inputBridge: { getText: () => string; setText: (v: string) => void } | null = null;
/** 录音开始时的输入框内容快照（拼接语义的基准：partial 只追加在这之后） */
let baseText = "";
/** 语音会话进行中（GameDialog 据此 readonly 输入框，语音期间禁止手动输入） */
export const asrVoiceActive = ref(false);
/** 功能开关（运行态）：auto_listen 模式开启时由 mic/快捷键切换——监听激活/暂停。
 *  不持久化、不改 auto_listen 模式设置。 */
const autoListenActive = ref(false);
/** 惰性依赖（首次 useAsrInput() 调用时初始化） */
let route: RouteLocationNormalizedLoaded | null = null;
let uiStore: ReturnType<typeof useUIStore> | null = null;
let asrStore: ReturnType<typeof useAsrStore> | null = null;
let gameStore: ReturnType<typeof useGameStore> | null = null;

// /chat（主界面）与 /pet（桌宠）都算聊天场景；设置抽屉打开时不可用
const chatActive = computed(() => {
  if (!route || !uiStore) return false;
  return (route.path === "/chat" || route.path === "/pet") && !uiStore.showSettings;
});

/** 拆除录音链路（不触发 recognize） */
function teardownRecorder() {
  try {
    processor?.disconnect();
  } catch {
    /* ignore */
  }
  processor = null;
  void audioCtx?.close().catch(() => {});
  audioCtx = null;
  stream?.getTracks().forEach((t) => t.stop());
  stream = null;
  pcmBuffer = [];
  vadPending = [];
  streamPending = [];
  vadSentFrames = 0;
  if (asrStore) asrStore.setMicState("idle");
}

/** 重置会话状态（录音拆除 + phase/activeSource 归位） */
function resetSession() {
  teardownRecorder();
  phase.value = "idle";
  asrVoiceActive.value = false;
  activeSource.value = null;
}

/**
 * 丢弃当前录音：停止本地采集但不触发识别（spec §3.0 —— 路由/抽屉离开时）。
 *
 * 注意：**在飞的云端识别不主动 cancel**（状态门控 plan §4 选 C）——
 * 让它自然完成，结果由 handle() 的 §4 判定（currentStatus ≠ input → drop）
 * 丢弃。之前这里对 recognizing 调 asrCancel()：用户发送消息后 AI 进入
 * thinking 会触发 updateAsrAvailability → discardRecording → 在飞识别被
 * 取消，用户白说（症状：[ASR] 识别失败: ASR 已取消）。
 */
function discardRecording() {
  const source = activeSource.value;
  // 流式会话清理：只丢流式句柄（不影响非流式在飞识别）
  void asrCancelStreaming();
  resetSession();
  if (source) void asrStopListening(source);
  // 会话被丢弃（路由/抽屉/TTS/触摸模式等门控打断）→ auto 触发标志必须复位，
  // 否则 autoTriggered 卡死 true → 能量监测永不触发（切界面后 auto_listen 失效）
  if (source === "auto") autoTriggered = false;
}

// ── ASR 可用性门控（§1 全 12 项） ──────────────────────────────
// 综合判定当前能否启动 ASR 录音（所有禁用条件取 OR）：
// 1-3. currentStatus ∈ {thinking, responding, presenting}
// 4.    command === 'touch'（触摸模式）
// 5.    showMobileMenu === true（移动端菜单展开）
// 6.    route.path !== '/chat'
// 7.    uiStore.showSettings === true
// 8.    runningScript && choices.length > 0（剧本选择分支）
// 9.    loadingComplete === false（启动动画未完成）
// 10.   显示锁未过期（识别结果填入后短暂禁止再触发；ignoreLock 供监测启停跳过）
// 11.   语音输入总开关关（自动与手动录音都被挡——总开关是整体语音输入开关）
// 12.   TTS 播放中（外放语音会被误识别）
// 任何一项满足即视为不可用。start() / startEnergyMonitor RMS 触发 / 按钮 enable 都查它。
// forManual=true（手动 mic 录音）：仅跳过 10 显示锁——锁防的是 auto 触发覆盖识别
// 结果（手动是用户主动，不受锁限）；总开关一律生效。
function canStartAsr(ignoreLock = false, forManual = false): boolean {
  if (!route || !uiStore || !gameStore) return false;
  // 6 + 7：路由/抽屉门控（chatActive 已是这两项的合成；/chat 与 /pet 均可）
  if ((route.path !== "/chat" && route.path !== "/pet") || uiStore.showSettings) return false;
  // 9：LoadingTransition 启动动画未完成（§1.9）
  if (!gameStore.loadingComplete) return false;
  // 1-3：核心对话状态
  if (gameStore.currentStatus !== "input") return false;
  // 4：触摸模式
  if (gameStore.command === "touch") return false;
  // 5：移动端菜单展开
  if (mobileMenuOpen) return false;
  // 8：剧本选择分支
  const script = (gameStore as unknown as { runningScript?: { choices?: unknown[] } })
    .runningScript;
  if (script && Array.isArray(script.choices) && script.choices.length > 0) return false;
  // 11：语音输入总开关——整体语音输入开关（自动与手动都被挡）
  if (!asrStore?.settings.voice_input_enabled) return false;
  // 12：角色语音（TTS）播放中（外放 TTS 进麦克风 → 误识别 AI 自己的话）
  if (voicePlaying.value) return false;
  // 10：识别结果短暂显示锁（fill_only 填入 inputMessage 到自动 send 的窗口期）。
  // ignoreLock=true 供 updateAsrAvailability 用：锁只挡"触发录音"，不挡"监测启停"——
  // 否则识别完成后锁一设监测就停、锁过期无人复活（触发后死锁）。
  // forManual=true（手动 mic）跳过锁：显示锁防的是 auto RMS 自动触发覆盖识别结果，
  // 手动点击是用户主动（fill_only 持续录入），不受锁限。
  if (!ignoreLock && !forManual && Date.now() < asrLockedUntil.value) return false;
  return true;
}

/** 同步录音 + 能量监测状态到最新可用性（任一 watch 触发时调用） */
function updateAsrAvailability(): void {
  // 监测启停不查显示锁（canStartAsr(true)）：锁只挡"触发录音"——识别完成后
  // 锁一设监测就停、锁过期无人复活，auto_listen 永久死锁（触发后死锁根因）
  const wantMonitor =
    canStartAsr(true) && (asrStore?.settings.auto_listen ?? false) && autoListenActive.value;
  if (wantMonitor) {
    startEnergyMonitor();
  } else {
    // 不可用 → 拆掉在飞录音 + 停能量监测。
    // 仅 recording 丢弃；recognizing 是收尾中（云端在飞识别不取消，§4 不变量），
    // 让 handle() 自然处理结果——否则关闭 auto_listen 会掐断正在识别的会话丢话。
    if (phase.value === "recording") {
      // 诊断：丢弃会话是"录音意外停止"的最可能路径，暴露触发原因
      console.log("[ASR] updateAsrAvailability 丢弃会话", {
        phase: phase.value,
        activeSource: activeSource.value,
        autoListen: asrStore?.settings.auto_listen,
      });
      discardRecording();
    }
    stopEnergyMonitor();
  }
}

/** GameDialog 调用：注册输入框读写桥（partial 写入 / 拼接基准） */
export function registerAsrInputBridge(b: {
  getText: () => string;
  setText: (v: string) => void;
}): void {
  inputBridge = b;
}

/** 流式是否生效：设置开关 + 当前生效模型的流式能力（模型级权威判定，
 *  元数据全部来自后端 asr_list_models——前端不再维护硬编码集合） */
function isStreamEnabled(): boolean {
  if (!asrStore?.settings.stream_enabled) return false;
  const sel = asrStore.settings.provider_configs[asrStore.settings.active_provider]?.model ?? "";
  const model =
    asrStore.models.find((m) => m.id === sel) ?? asrStore.models.find((m) => m.is_default);
  // 模型清单未加载（拉取失败等）时流式判定为 false → 走整句识别；
  // 配置了流式模型却降级整句的代价是"无 partial"，后端能力不受影响
  const enabled = model?.supports_streaming ?? false;
  return enabled;
}

/** SSE 类结果流式 provider 判定（llama-asr 起步；名字带 llama 是历史遗留）。
 *  语义：整段 WAV 上传 + SSE 增量 partial（stop 后到达），与 qwen WS 真流式
 *  （边录边发 PCM）分流。
 *
 *  ⚠️ 扩展点：新增 SSE 类 provider（如未来的 openai-compatible 泛化入口，
 *  见 provider.rs 扩展指南）必须同步加进此集合——llama-asr 与它同链路
 *  （不建 WS 会话、stop 时整段上传、partial 在 recognizing 阶段放行）。 */
function isLlamaStream(): boolean {
  const sseProviders = ["llama-asr"];
  return sseProviders.includes(asrStore?.settings.active_provider ?? "") && isStreamEnabled();
}

/** GameDialog 调用：同步移动端菜单展开状态（§1.5） */
export function setMobileMenuOpen(open: boolean): void {
  mobileMenuOpen = open;
  updateAsrAvailability();
}

/** GameRolesStage（桌面/桌宠）调用：同步角色语音播放状态。
 *  TTS 播放开始 → 停能量监测 + 丢弃在飞 auto 录音（那是在录 AI 的声音）；
 *  播放结束 → 恢复监听。 */
export function setVoicePlaying(playing: boolean): void {
  voicePlaying.value = playing;
  updateAsrAvailability();
}

/**
 * GameDialog 调用：锁定 ASR 一段时间（识别结果填入 inputMessage 后短暂显示用，§1.10）。
 * 显示期间用户不能再次触发录音（避免 nextTick 期间又来一段覆盖识别结果）。
 */
export function lockAsrForDisplay(ms: number): void {
  asrLockedUntil.value = Date.now() + ms;
  updateAsrAvailability();
}

/** GameDialog / ChatInput（桌宠）调用：模式开时切换功能开关（暂停/恢复自动监听）。
 *  只动运行态 autoListenActive，不改 auto_listen 模式设置（无 save）。 */
function toggleAutoListenFunction() {
  if (autoListenActive.value) {
    // 暂停：auto 录音中先收尾识别（不丢话），updateAsrAvailability 对
    // recognizing 不丢弃，识别结果照常按 send_mode 处理
    if (phase.value === "recording" && activeSource.value === "auto") {
      stop();
    }
    autoListenActive.value = false;
  } else {
    autoListenActive.value = true;
  }
  updateAsrAvailability();
}

// ── VAD 流（auto 模式）：每 512 samples（30ms @ 16k）喂后端 ──
// 严格串行单飞：一块 invoke 完成才发下一块。Silero 的 h/c 隐状态依赖
// 顺序输入——并发 fire-and-forget 会导致后端锁等待乱序，prob 结果无意义
// （表现：VAD 永不触发 SpeechStarted / TurnCandidate）。
let vadSending = false;
/** 诊断：已发送的 VAD 块数（用于降频日志） */
let vadSentFrames = 0;
function feedVad() {
  if (!asrStore || phase.value !== "recording" || activeSource.value !== "auto") return;
  if (vadSending || vadPending.length < 512) return;
  const block = vadPending.splice(0, 512);
  vadSending = true;
  // 诊断日志：前 10 块 + 每秒 1 条（33 块），确认 VAD 流在走
  // if (vadSentFrames < 10 || vadSentFrames % 33 === 0) {
  //   console.log(`[ASR/VAD] feedVad #${vadSentFrames} 发送 ${block.length} samples`);
  // }
  vadSentFrames++;
  asrVadProcessChunk(block)
    .catch((e) => {
      // VAD 失败不阻塞录音，但错误不能静默——暴露给调试者
      console.warn("[ASR/VAD] feedVad 失败:", e);
    })
    .finally(() => {
      vadSending = false;
      feedVad();
    });
}

// ── 流式识别音频流（stream 模式）：与 VAD 同节奏喂后端 WebSocket ──
// 与 feedVad 相同串行单飞：invoke 不保证顺序，WebSocket 帧必须保序。
let streamPending: number[] = [];
let streamSending = false;
function feedStream() {
  if (!asrStore || phase.value !== "recording") return;
  if (streamSending || streamPending.length < 512) return;
  const block = streamPending.splice(0, 512);
  streamSending = true;
  asrStreamAudioChunk(block)
    .catch((e) => console.warn("[ASR/stream] 发送音频块失败:", e))
    .finally(() => {
      streamSending = false;
      feedStream();
    });
}

/** VAD 检测到一轮说话结束（turn_candidate / turn_sealed）→ 结束 auto 会话 */
async function onVadTurnEnd() {
  console.log("[ASR] VAD turn 事件, activeSource=", activeSource.value, "phase=", phase.value);
  if (activeSource.value !== "auto") return;
  if (phase.value === "recording") {
    stop();
  }
}

// ── 能量监测（auto_listen 常开，RMS 超阈值触发 auto 会话） ──
function startEnergyMonitor() {
  if (energyMon) return;
  // §1 全 12 项 + auto_listen 设置：任何一项不满足则不开
  if (!asrStore?.settings.auto_listen) return;
  if (!canStartAsr()) return;
  console.log("[ASR] startEnergyMonitor 启动 (auto_listen=on, canStartAsr=true)");
  navigator.mediaDevices
    .getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } })
    .then((s) => {
      if (!asrStore?.settings.auto_listen || !chatActive.value) {
        console.log("[ASR] startEnergyMonitor 启动后条件失效，关闭 stream");
        s.getTracks().forEach((t) => t.stop());
        return;
      }
      const ctx = new AudioContext();
      const src = ctx.createMediaStreamSource(s);
      const analyser = ctx.createAnalyser();
      analyser.fftSize = 1024;
      analyser.smoothingTimeConstant = 0.3;
      src.connect(analyser);
      const buf = new Uint8Array(analyser.frequencyBinCount);
      // 启动缓冲期：从 analyser 建立起算，头 N 毫秒不触发录音
      // （N = 设置页 energy_warmup_ms，兜底 TTS 播完瞬间的残响尾巴，0=无缓冲）
      const warmupUntil = Date.now() + (asrStore?.settings.energy_warmup_ms ?? ENERGY_WARMUP_MS);
      const tick = () => {
        if (!asrStore?.settings.auto_listen || !chatActive.value) {
          stopEnergyMonitor();
          return;
        }
        if (!energyMon) return;
        if (Date.now() < warmupUntil) {
          energyMon.raf = requestAnimationFrame(tick);
          return;
        }
        analyser.getByteFrequencyData(buf);
        // RMS 归一化：byte 0-255 → 0-1，阈值 0.08 约等于明显人声能量
        let sum = 0;
        for (let i = 0; i < buf.length; i++) sum += buf[i] * buf[i];
        const rms = Math.sqrt(sum / buf.length) / 128;
        if (rms > 0.08 && phase.value === "idle" && !autoTriggered) {
          // 二次校验：AI 可能在本帧之间从 input 进入 thinking，RMS 触发时已不可用
          if (!canStartAsr()) {
            energyMon.raf = requestAnimationFrame(tick);
            return;
          }
          autoTriggered = true;
          void start("auto").catch((err) => {
            console.warn("[ASR] start(auto) failed, reset autoTriggered:", err);
            autoTriggered = false;
          });
          return;
        }
        energyMon.raf = requestAnimationFrame(tick);
      };
      energyMon = { ctx, raf: requestAnimationFrame(tick), stream: s };
      console.log("[ASR] startEnergyMonitor 已建立 analyser, tick loop 开始");
    })
    .catch((err) => {
      console.warn("[ASR] startEnergyMonitor getUserMedia 失败:", err);
      /* mic 不可用：能量监测静默降级 */
    });
}

function stopEnergyMonitor() {
  if (!energyMon) return;
  cancelAnimationFrame(energyMon.raf);
  void energyMon.ctx.close().catch(() => {});
  energyMon.stream.getTracks().forEach((t) => t.stop());
  energyMon = null;
}

// ── 会话生命周期 ────────────────────────────────────────────
async function start(source: AsrSource) {
  // §1 全 12 项门控；手动模式（button）跳过显示锁（总开关一律生效）
  if (!canStartAsr(false, source === "button")) {
    // 诊断：静默拒绝会让按钮"按下无反应"，暴露拒绝原因
    console.log("[ASR] start 被门控拒绝", {
      source,
      phase: phase.value,
      status: gameStore?.currentStatus,
      command: gameStore?.command,
      loadingComplete: gameStore?.loadingComplete,
      locked: Date.now() < asrLockedUntil.value,
    });
    return;
  }
  if (activeSource.value !== null) {
    throw new Error("ASR session busy");
  }
  activeSource.value = source;
  phase.value = "recording";
  asrVoiceActive.value = true;
  asrStore?.setMicState("recording");
  try {
    // 拼接基准：录音开始时的输入框内容（仅按钮源可拼接，auto 统一处理）
    baseText = inputBridge?.getText() ?? "";
    // 流式：先建 WebSocket（互斥由后端 stream 检查 + start_listening 的 active 检查双层保证）。
    // llama-asr 结果流式不建 WS（stop 时整段上传），仅 qwen WS 真流式走这里
    if (isStreamEnabled() && !isLlamaStream()) {
      await asrStartStreaming({
        providerId: asrStore?.settings.active_provider ?? "openai-whisper",
        languageHint: null,
      });
    }
    stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        sampleRate: 16000,
        channelCount: 1,
        echoCancellation: true,
        noiseSuppression: true,
      },
    });
    audioCtx = new AudioContext({ sampleRate: 16000 });
    const src = audioCtx.createMediaStreamSource(stream);
    processor = audioCtx.createScriptProcessor(1024, 1, 1);
    src.connect(processor);
    // 输出接零增益节点而非 destination，避免把采集流回放
    const silence = audioCtx.createGain();
    silence.gain.value = 0;
    processor.connect(silence);
    silence.connect(audioCtx.destination);
    processor.onaudioprocess = (e) => {
      const data = e.inputBuffer.getChannelData(0);
      pcmBuffer.push(...data);
      if (source === "auto") {
        vadPending.push(...data);
        // 上限保护：串行速率低于产生速率时丢弃最旧（8192 块 ≈ 4 分钟音频，
        // VAD 端点检测只需要最近的音频）
        if (vadPending.length > 8192) {
          vadPending.splice(0, vadPending.length - 8192);
        }
        feedVad();
      }
      if (isStreamEnabled() && !isLlamaStream()) {
        streamPending.push(...data);
        // 与 vadPending 同思路的上限保护（8192 块 ≈ 4 分钟音频）
        if (streamPending.length > 8192) {
          streamPending.splice(0, streamPending.length - 8192);
        }
        feedStream();
      }
      // 录音硬上限（1 分钟）：达到自动停止。放回调末尾——stop() 会取走
      // pcmBuffer 合成 WAV，此前的数据完整保留；VAD/流式块已在此前送完，
      // 不再残留
      if (pcmBuffer.length >= MAX_RECORD_SAMPLES) {
        stop();
      }
    };
    await asrStartListening(source);
  } catch (err: unknown) {
    const name = (err as { name?: string }).name;
    console.warn("[ASR] start failed:", err);
    if (name === "NotAllowedError" || name === "NotReadableError") {
      asrStore?.setMicState("denied");
      asrStore?.onError("ASR_MIC_DENIED");
    } else {
      asrStore?.onError(parseAsrError(err).code || String(err));
    }
    // 流式 WebSocket 可能已建立（getUserMedia / startListening 失败路径）：
    // 必须清理，否则后端句柄残留 → 下次启动 SessionBusy
    void asrCancelStreaming();
    resetSession();
    throw err;
  }
}

/** 手动结束（mic 按钮 / 快捷键松开 / VAD turn 结束）：停止 → 识别 → 处理 */
function stop() {
  if (phase.value !== "recording") return;
  const source = activeSource.value;
  if (!source) return;
  phase.value = "recognizing";
  // 先拿走 PCM 再拆录音链路（teardownRecorder 会清空 pcmBuffer）
  const captured = pcmBuffer;
  teardownRecorder();
  void asrStopListening(source);
  if (isStreamEnabled() && !isLlamaStream()) {
    void doStreamFinish(source);
  } else {
    // 非流式 + llama 结果流式都走整句上传（后者命令不同，内部按 provider 分派）
    void doRecognize(source, captured);
  }
}

/** 流式收尾：stop → 等整段 final → handle（与非流式同链路） */
async function doStreamFinish(source: AsrSource) {
  try {
    const result = await asrStopStreaming();
    handle(result.text, source);
  } catch (err) {
    console.error("[ASR/stream] 收尾失败:", err);
    // 错误链路打通：设置页状态面板 + mic 按钮可感知识别失败（架构 A）
    asrStore?.onError(parseAsrError(err).code || String(err));
    resetSession();
    if (source === "auto") {
      autoTriggered = false;
      updateAsrAvailability();
    }
  }
}

/** 把录音 PCM 合成 WAV 送识别，成功后 handle()。
 *  llama-asr 结果流式（流式开关开启）时走 asr_recognize_wav_stream——
 *  整段上传后由后端 SSE partial 事件刷输入框，本函数只等 final。 */
async function doRecognize(source: AsrSource, captured: number[]) {
  try {
    // 裁剪首尾静音：录音含触发前的环境声 + VAD 停顿尾巴，只送语音段
    const trimmed = trimSilencePcm(captured);
    const wav = pcmToWavPcm16(trimmed);
    if (wav.byteLength <= 44) {
      // 纯静音（无采样）：直接放弃，不浪费一次识别调用
      resetSession();
      if (source === "auto") {
        autoTriggered = false;
        updateAsrAvailability();
      }
      return;
    }
    const providerId = asrStore?.settings.active_provider ?? "openai-whisper";
    const result = isLlamaStream()
      ? await asrRecognizeWavStream({ providerId, wavBytes: Array.from(wav) })
      : await asrRecognizeWav({ providerId, wavBytes: Array.from(wav), languageHint: null });
    handle(result.text, source);
  } catch (err) {
    console.error("[ASR] recognize failed:", err);
    // 错误链路打通：设置页状态面板 + mic 按钮可感知识别失败（架构 A）
    asrStore?.onError(parseAsrError(err).code || String(err));
    resetSession();
    if (source === "auto") {
      autoTriggered = false;
      updateAsrAvailability();
    }
  }
}

/**
 * 识别后处理：填入 / 渲染后延迟发送
 * 两模式（asrStore.settings.send_mode）：
 * - fill_only: emit window 'asr-text' event，GameDialog 监听后填 inputMessage
 * - auto_send: 识别内容完整渲染到聊天（与手动发送一致），800ms 后 invoke
 *   send_chat_message（AI 忙时由后端 generation_lock 排队，无需前端降级）
 */
function handle(text: string, source: AsrSource) {
  // §4: 识别请求在飞行中 AI 可能从 input 进入 thinking/responding/presenting
  // 返回时 currentStatus 已变 → 识别结果丢弃（不填入 / 不发送 / 不入队）。
  // voicePlaying：手动模式点击继续后 TTS 还在播，在飞识别（误录的 AI 语音）
  // 返回时同样丢弃。!chatActive：识别期间已切界面/打开设置抽屉 → 结果丢弃
  // （用户方案：没说完不发送，回来点 mic 重新启用）。
  if (
    !gameStore ||
    gameStore.currentStatus !== "input" ||
    voicePlaying.value ||
    !chatActive.value
  ) {
    resetSession();
    if (source === "auto") {
      autoTriggered = false;
      updateAsrAvailability();
    }
    return;
  }
  // 空识别结果（云端未识别出内容）：静默复位 + 重启监听，不 dispatch / 不发送——
  // 否则 fill_only 空串覆盖输入框（清空用户草稿），auto_send 空消息报后端"消息内容不能为空"。
  if (!text.trim()) {
    console.log("[ASR] handle: 空识别结果，复位会话并重启监听");
    resetSession();
    if (source === "auto") {
      autoTriggered = false;
      updateAsrAvailability();
    }
    return;
  }
  const mode = asrStore?.settings.send_mode ?? "fill_only";
  // 拼接只对手动录音（button 源）+ fill_only 生效：识别结果追加到录音开始时的
  // 输入框内容（baseText）之后，持续录入不覆盖。auto 源与 auto_send 不拼接
  // （auto_send 只发送识别内容本身，不做与已有内容的衔接）。
  const full = source === "button" && mode === "fill_only" ? baseText + text : text;
  if (mode === "fill_only") {
    window.dispatchEvent(new CustomEvent("asr-text", { detail: full }));
  } else if (mode === "auto_send") {
    // 事件驱动组件发送链路（GameDialog / ChatInput 监听 'asr-send'）：
    // 组件负责 setText 显示完整结果 → ASR_AUTO_SEND_DELAY_MS 后走各自完整
    // send()——复用剧本分支（runningScript → script_submit_input）、模型配置
    // 检查与输入框清理，避免这里直接 invoke send_chat_message 绕过剧本引擎
    // （剧本自由对话模式下消息会发进主 LLM 而非剧本引擎）。
    // 显示锁直接赋值 asrLockedUntil 而非 lockAsrForDisplay()：handle 执行时
    // phase 尚在 'recognizing'，lockAsrForDisplay → updateAsrAvailability
    // 会误判丢弃会话（递归）。
    window.dispatchEvent(new CustomEvent("asr-send", { detail: full }));
    asrLockedUntil.value = Date.now() + ASR_AUTO_SEND_DELAY_MS;
  }
  resetSession();
  // auto 模式本轮结束：复位触发标志 + 通过统一门控重新评估能量监测
  if (source === "auto") {
    autoTriggered = false;
    updateAsrAvailability();
  }
}

// ── 惰性初始化（首次调用时执行一次，注册全局监听） ──────────
let initialized = false;
function ensureInit() {
  if (initialized) return;
  initialized = true;
  route = useRoute();
  uiStore = useUIStore();
  asrStore = useAsrStore();
  gameStore = useGameStore();

  // 与后端同步设置：store 可能被 persist 恢复了 localStorage 旧值
  // （如旧 active_provider），不 load 会导致识别走到错误的 provider。
  // load 完成后热键/auto_listen 的 watch 会自动响应新值。
  void asrStore.load().catch((e) => console.warn("[ASR] load settings failed:", e));

  // VAD 事件（经 store 中转，与 tauri-events.ts 的全局监听共用 store 字段）
  watch(
    () => asrStore?.vadEvent ?? null,
    (e: VadEvent | null) => {
      if (!e) return;
      if (e.type === "turn_candidate" || e.type === "turn_sealed") {
        void onVadTurnEnd();
      }
    }
  );

  // 流式 partial：实时写入输入框（整体替换语音追加块，不触碰 baseText 之前的内容）
  listen("asr://stream_partial", (e) => {
    // 诊断：暴露 partial 是否到达前端、写入条件（phase/inputBridge）是否满足
    // 写入条件：qwen WS 真流式在录音期间到达（phase=recording）；llama 结果
    // 流式（SSE）在 stop() 之后到达（phase=recognizing）——必须放行，
    // 否则 llama 的增量 partial 全部被丢弃（v2 流式功能失效）
    const writeOk =
      phase.value === "recording" || (isLlamaStream() && phase.value === "recognizing");
    if (writeOk && typeof e.payload === "string") {
      inputBridge?.setText(baseText + e.payload);
    }
  });

  // 路由/抽屉变化（§1.6/7）：通过统一 gate 同步录音/能量监测
  // immediate:true 让首次进入 /chat（或刚初始化）时立刻同步 energy monitor 状态
  watch(
    chatActive,
    (active) => {
      if (!active) {
        // 切界面（路由离开 /chat+/pet / 设置抽屉打开）= 等同 mic 关闭：
        // 暂停 auto 监听（回来需点 mic 重新启用），在飞识别结果由
        // handle 的 chatActive 检查丢弃（"没说完不发送"）
        autoListenActive.value = false;
      }
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // auto_listen 设置开关（用户在设置页切换时立即启停）
  watch(
    () => asrStore?.settings.auto_listen,
    (enabled) => {
      console.log(`[ASR] auto_listen -> ${enabled}`);
      // 模式开关：开 = 功能默认激活；关 = 功能复位（功能开关只在模式开时有意义）
      autoListenActive.value = !!enabled;
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // 语音输入总开关（设置页切换立即生效）
  watch(
    () => asrStore?.settings.voice_input_enabled,
    (enabled) => {
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // 触摸模式（§1.4）
  watch(
    () => gameStore?.command,
    (cmd) => {
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // currentStatus（§1.1-3：thinking/responding/presenting）
  watch(
    () => gameStore?.currentStatus,
    (status) => {
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // 剧本选择分支（§1.8）
  watch(
    () =>
      (gameStore as unknown as { runningScript?: { choices?: unknown[] } })?.runningScript?.choices
        ?.length ?? 0,
    (n) => {
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // LoadingTransition 启动动画完成（§1.9）
  watch(
    () => gameStore?.loadingComplete,
    (done) => {
      updateAsrAvailability();
    },
    { immediate: true }
  );
}

export function useAsrInput() {
  ensureInit();
  return {
    phase,
    activeSource,
    chatActive,
    start,
    stop,
    discardRecording,
    handle,
    cancel: () => asrCancel(),
    canStartAsr,
    autoListenActive,
    toggleAutoListenFunction,
  };
}

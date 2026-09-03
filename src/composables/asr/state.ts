import { ref, shallowRef } from "vue";
import type { RouteLocationNormalizedLoaded } from "vue-router";
import type { AsrSource } from "@/api/services/asr";
import type { useGameStore } from "@/stores/modules/game";
import type { useAsrStore } from "@/stores/modules/settings/asr";
import type { useUIStore } from "@/stores/modules/ui/ui";

/** 输入框读写桥（GameDialog / ChatInput 注册，供 partial 写入 / 拼接基准读取） */
export interface AsrInputBridge {
  getText: () => string;
  setText: (v: string) => void;
}

/** 能量监测句柄（raf 循环 + 独立采集流，stop 时统一回收） */
export interface EnergyMonitorHandle {
  ctx: AudioContext;
  raf: number;
  stream: MediaStream;
}

// ── 模块级单例状态 ──────────────────────────────────────────
// 状态全部在模块级（非函数内）：App.vue 的初始化实例与 GameDialog /
// ChatInput 的 mic 实例共享同一会话。
//
// 两种形态：
// - ref 类（响应式，export const）：phase/activeSource/voicePlaying 等，
//   消费方经 `.value` 读写（ref 对象本身不变，import 绑定只读无碍）
// - 非响应式可变状态集中在 runtime 容器（export const 对象，成员可变）：
//   ES module 的 import 绑定只读，跨模块直接赋值非法——容器成员是
//   跨模块共享可写状态的唯一合法形态（模块化重构，逻辑不变）
export const phase = ref<"idle" | "recording" | "recognizing">("idle");
export const activeSource = shallowRef<AsrSource | null>(null);

export const runtime = {
  /** 本次录音累积的 f32 PCM（16kHz mono） */
  pcmBuffer: [] as number[],
  /** 待喂 VAD 的积累块（凑满 512 samples = 30ms 才发） */
  vadPending: [] as number[],
  /** 待喂流式 WS 的积累块（与 vadPending 同节奏，session/recorder 共享） */
  streamPending: [] as number[],
  stream: null as MediaStream | null,
  audioCtx: null as AudioContext | null,
  processor: null as ScriptProcessorNode | null,
  energyMon: null as EnergyMonitorHandle | null,
  /** auto 触发去重：能量触发后不再重复触发，直到本轮会话结束 */
  autoTriggered: false,
  /** 移动端菜单展开状态（GameDialog 在 watch 中同步，§1.5 判定） */
  mobileMenuOpen: false,
  /** 输入框桥：GameDialog 注册，供 partial 实时写入 / 拼接基准读取 */
  inputBridge: null as AsrInputBridge | null,
  /** 录音开始时的输入框内容快照（拼接语义的基准：partial 只追加在这之后） */
  baseText: "",
  /** 全局快捷键实际注册状态（审查中危 1）：asr:ptt-global-status 事件 / asrGetStatus
   *  查询驱动。退位判断用它而非设置值——注册失败（键被占用）时窗口内监听继续工作，
   *  避免"设置开但全局没注册 + 窗口内退位"的双重失效（重启后 PTT 完全静默） */
  pttGlobalOk: true,
  /** 是否已收到过状态事件（审查 P2 时序防护）：启动时 asrGetStatus 异步返回的是
   *  旧状态，若期间收到保存成功事件（ok:true），查询后到会错误覆盖——查询结果
   *  仅在从未收到事件时生效（事件总是更新的） */
  pttStatusEventSeen: false,
  /** 惰性依赖（首次 useAsrInput() 调用时初始化） */
  route: null as RouteLocationNormalizedLoaded | null,
  uiStore: null as ReturnType<typeof useUIStore> | null,
  asrStore: null as ReturnType<typeof useAsrStore> | null,
  gameStore: null as ReturnType<typeof useGameStore> | null,
};

/** 短暂显示锁：识别后填入 inputMessage 到自动 send 之间的窗口期，期间 auto 触发禁用（§1.10）。
 *  ref 化（非普通变量）：canStartMic 等 computed 依赖它，锁过期后能自动重算解锁。 */
export const asrLockedUntil = ref(0);
/** 录音硬上限（samples）：1 分钟 @ 16kHz。达到后自动 stop()——
 *  防止按钮长按/异常会话无限录音（VAD 端 max_segment_frames 同为 60s，两处对齐；
 *  有界也顺带解决长时间录音时 pcmBuffer 的无限内存增长）。 */
export const MAX_RECORD_SAMPLES = 60 * 16000;
/** 能量监测启动缓冲期兜底值（毫秒）：未加载设置时用 100ms。
 *  实际值来自 asrStore.settings.energy_warmup_ms（设置页可自定义，
 *  0 = 无缓冲）。voicePlaying 门控已保证 TTS 播放期间完全不监听，
 *  此缓冲期只兜底播放结束瞬间的残响尾巴。 */
export const ENERGY_WARMUP_MS = 100;
/** 角色语音（TTS）播放中（GameRolesStage 桌面/桌宠通过 setVoicePlaying 同步）：
 *  外放 TTS 会被麦克风捕获 → RMS 触发 → VAD 判定为人声 → 误识别 AI 自己的话。
 *  播放期间 ASR 整体禁用（canStartAsr 门控 + handle drop），播完才恢复。
 *  ref 化：canStartMic 等 computed 依赖它，播完 setVoicePlaying(false) 自动解锁。 */
export const voicePlaying = ref(false);
/** 语音会话进行中（GameDialog 据此 readonly 输入框，语音期间禁止手动输入）。
 *  前缀 asr 用于与同模块的 voicePlaying（TTS 播放中）在组件顶层导入时消歧。 */
export const asrVoiceActive = ref(false);
/** 功能开关（运行态）：auto_listen 模式开启时由 mic/快捷键切换——监听激活/暂停。
 *  不持久化、不改 auto_listen 模式设置。 */
export const autoListenActive = ref(false);

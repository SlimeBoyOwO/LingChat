import { watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { useRoute } from "vue-router";

import { asrLog } from "./log";
import { canStartAsr, chatActive, isLlamaStream } from "./gates";
import { writePartial, registerAsrInputBridge } from "./input-bridge";
import {
  onVadTurnEnd,
  start,
  stop,
  discardRecording,
  handle,
  updateAsrAvailability,
  toggleAutoListenFunction,
} from "./session";
import { handlePttKeyDown, handlePttKeyUp, handleWindowBlur, pttDown, pttUp } from "./ptt";
import { ASR_AUTO_SEND_DELAY_MS, useAsrAutoSend } from "./auto-send";
export { ASR_AUTO_SEND_DELAY_MS, useAsrAutoSend };
import {
  activeSource,
  asrLockedUntil,
  asrVoiceActive,
  autoListenActive,
  phase,
  runtime,
  voicePlaying,
} from "./state";
export { asrVoiceActive, registerAsrInputBridge };
import { asrCancel, asrGetStatus, type VadEvent } from "@/api/services/asr";
import { useGameStore } from "@/stores/modules/game";
import { useAsrStore } from "@/stores/modules/settings/asr";
import { useUIStore } from "@/stores/modules/ui/ui";
import { isAndroid } from "@/utils/platform";

/**
 * 统一 ASR 输入入口（门面）：两种触发源共用同一会话生命周期。
 *
 * 两种触发源：
 * - Button: GameDialog.vue / ChatInput.vue（桌宠）的 mic 按钮
 * - Auto: asrStore.settings.auto_listen=true 时由能量监测触发
 *
 * 窗口活跃门控：仅当 chatActive=true（/chat 或 /pet 路由 + 设置抽屉未开）时启用。
 * 失败降级：mic 不可用时 fail-open（不抛错到用户），退化为手动按钮 + 不录。
 *
 * ── 单例设计 ──────────────────────────────────────────────
 * 状态全部在模块级（asr/state.ts）：App.vue 的初始化实例与 GameDialog /
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

/** GameDialog 调用：同步移动端菜单展开状态（§1.5） */
export function setMobileMenuOpen(open: boolean): void {
  runtime.mobileMenuOpen = open;
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

// ── 惰性初始化（首次调用时执行一次，注册全局监听） ──────────
let initialized = false;
function ensureInit() {
  if (initialized) return;
  initialized = true;
  runtime.route = useRoute();
  runtime.uiStore = useUIStore();
  runtime.asrStore = useAsrStore();
  runtime.gameStore = useGameStore();

  // 与后端同步设置：store 可能被 persist 恢复了 localStorage 旧值
  // （如旧 active_provider），不 load 会导致识别走到错误的 provider。
  // load 完成后热键/auto_listen 的 watch 会自动响应新值。
  void runtime.asrStore.load().catch((e) => asrLog().warn("load settings failed:", e));

  // VAD 事件（经 store 中转，与 tauri-events.ts 的全局监听共用 store 字段）
  watch(
    () => runtime.asrStore?.vadEvent ?? null,
    (e: VadEvent | null) => {
      if (!e) return;
      if (e.type === "turn_candidate" || e.type === "turn_sealed") {
        void onVadTurnEnd();
      }
    }
  );

  // 流式 partial：实时写入输入框（整体替换语音追加块，不触碰 baseText 之前的内容）
  // 诊断降频（审查：partial 每块到达即打会刷屏）：前 10 条 + 此后每 33 条 1 条
  //（≈每秒 1 条，与 feedVad 日志同节奏）
  let partialLogCount = 0;
  listen("asr://stream_partial", (e) => {
    if (partialLogCount < 10 || partialLogCount % 33 === 0) {
      // 诊断：暴露 partial 是否到达前端、写入条件（phase/inputBridge）是否满足
      asrLog("[ASR/stream]").info(
        `partial 事件: len=${String(e.payload).length}, phase=${phase.value}, ` +
          `bridge=${runtime.inputBridge ? "ok" : "null"}`
      );
    }
    partialLogCount++;
    // 写入条件：qwen WS 真流式在录音期间到达（phase=recording）；llama 结果
    // 流式（SSE）在 stop() 之后到达（phase=recognizing）——必须放行，
    // 否则 llama 的增量 partial 全部被丢弃（v2 流式功能失效）
    const writeOk =
      phase.value === "recording" || (isLlamaStream() && phase.value === "recognizing");
    if (writeOk && typeof e.payload === "string") {
      writePartial(e.payload);
    }
  });

  // 按住说话(PTT)：keydown/keyup 开始/结束录音；blur 兜底（失焦丢 keyup）。
  // ensureInit 仅主窗口执行一次（App.vue isMainWindow 分支 + GameDialog/ChatInput 共享），
  // /chat 与 /pet 路由自动覆盖。安卓端不做快捷键：蓝牙键盘的 keydown 同样会
  // 到达 webview（注释"无物理键盘"不成立），显式不注册监听；全局快捷键
  // 后端已 #[cfg(desktop)] 不编译，两侧都干净
  if (!isAndroid()) {
    window.addEventListener("keydown", handlePttKeyDown);
    window.addEventListener("keyup", handlePttKeyUp);
    window.addEventListener("blur", handleWindowBlur);
  }

  // 全局快捷键（失去焦点可用）：插件已匹配键位（后端 asr:ptt-global 仅在注册成功后
  // 才有事件），按下/释放直接驱动状态机；总开关/路由/AI 生成等门控由 pttDown/pttUp
  // 内既有检查把关（与窗口内完全一致）。移动端后端不注册该事件源，天然不触发
  listen<{ state: "pressed" | "released" }>("asr:ptt-global", (e) => {
    if (e.payload.state === "pressed") {
      pttDown();
    } else {
      pttUp();
    }
  });

  // 全局注册状态：失败事件（后端仅失败时 emit）→ pttGlobalOk=false → 窗口内监听
  // 兜底；设置保存成功时后端 emit ok:true 复位（审查中危 1）
  listen<{ ok: boolean; reason: string }>("asr:ptt-global-status", (e) => {
    runtime.pttStatusEventSeen = true;
    runtime.pttGlobalOk = e.payload.ok;
  });
  // 启动时查询式初始化：重启后注册失败（启动 sync 失败只 warn 不 emit 事件）→
  // pttGlobalOk=false → 窗口内监听兜底，不再"双重失效"静默。
  // 仅当从未收到事件时生效（P2 时序防护：查询可能返回旧状态，不能覆盖事件）
  asrGetStatus()
    .then((s) => {
      if (!runtime.pttStatusEventSeen) runtime.pttGlobalOk = !!s.ptt_global_ok;
    })
    .catch(() => {
      /* 查询失败保持默认 true，退位判断保守 */
    });

  // 路由/抽屉变化（§1.6/7）：通过统一 gate 同步录音/能量监测
  // immediate:true 让首次进入 /chat（或刚初始化）时立刻同步 energy monitor 状态
  watch(
    chatActive,
    (active) => {
      asrLog().info(`chatActive -> ${active}`);
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
    () => runtime.asrStore?.settings.auto_listen,
    (enabled) => {
      asrLog().info(`auto_listen -> ${enabled}`);
      // 模式开关：开 = 功能默认激活；关 = 功能复位（功能开关只在模式开时有意义）
      autoListenActive.value = !!enabled;
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // 语音输入总开关（设置页切换立即生效）
  watch(
    () => runtime.asrStore?.settings.voice_input_enabled,
    (enabled) => {
      asrLog().info(`voice_input_enabled -> ${enabled}`);
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // 触摸模式（§1.4）
  watch(
    () => runtime.gameStore?.command,
    (cmd) => {
      asrLog().info(`command -> ${cmd}`);
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // currentStatus（§1.1-3：thinking/responding/presenting）
  watch(
    () => runtime.gameStore?.currentStatus,
    (status) => {
      asrLog().info(`currentStatus -> ${status}`);
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // 剧本选择分支（§1.8）
  watch(
    () =>
      (runtime.gameStore as unknown as { runningScript?: { choices?: unknown[] } })?.runningScript
        ?.choices?.length ?? 0,
    (n) => {
      asrLog().info(`runningScript.choices.length -> ${n}`);
      updateAsrAvailability();
    },
    { immediate: true }
  );
  // LoadingTransition 启动动画完成（§1.9）
  watch(
    () => runtime.gameStore?.loadingComplete,
    (done) => {
      asrLog().info(`loadingComplete -> ${done}`);
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

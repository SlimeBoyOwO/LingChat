import { asrLog } from "./log";
import { canStartAsr, chatActive, isLlamaStream, isStreamEnabled } from "./gates";
import { setAutoStartHandler, startEnergyMonitor, stopEnergyMonitor } from "./energy-monitor";
import { resetPtt } from "./ptt-state";
import {
  MAX_RECORD_SAMPLES,
  activeSource,
  asrLockedUntil,
  asrVoiceActive,
  autoListenActive,
  phase,
  runtime,
  voicePlaying,
} from "./state";
import { ASR_AUTO_SEND_DELAY_MS } from "./auto-send";
import { feedStream, feedVad, teardownRecorder } from "./recorder";
import {
  asrCancelStreaming,
  asrRecognizeWav,
  asrRecognizeWavStream,
  asrStartListening,
  asrStartStreaming,
  asrStopListening,
  asrStopStreaming,
  type AsrSource,
} from "@/api/services/asr";
import { pcmToWavPcm16, trimSilencePcm } from "@/utils/asrAudio";
import { parseAsrError } from "@/utils/asrError";

// energy-monitor 的 RMS 触发经回调注入（模块化防循环：energy-monitor 不反向
// 依赖 session；模块加载时注册，start 为函数声明提升可用）
setAutoStartHandler((source) => start(source));

/** 重置会话状态（录音拆除 + phase/activeSource 归位） */
function resetSession() {
  teardownRecorder();
  phase.value = "idle";
  asrVoiceActive.value = false;
  activeSource.value = null;
  // PTT 手势状态随会话复位：任何 teardown 路径（discard/handle/识别错误/start 失败）
  // 都不残留 toggle-held，避免后续快捷键/blur 把 PTT 手势误判到 mic 按钮的会话上
  resetPtt();
}

/**
 * 丢弃当前录音：停止本地采集但不触发识别（spec §3.0 —— 路由/抽屉离开时）。
 *
 * 注意：**在飞的云端识别不主动 cancel**（状态门控 plan §4 选 C）——
 * 让它自然完成，结果由 handleResult() 的 §4 判定（currentStatus ≠ input → drop）
 * 丢弃。之前这里对 recognizing 调 asrCancel()：用户发送消息后 AI 进入
 * thinking 会触发 updateAsrAvailability → discardRecording → 在飞识别被
 * 取消，用户白说（症状：[ASR] 识别失败: ASR 已取消）。
 */
export function discardRecording() {
  const source = activeSource.value;
  // 流式会话清理：只丢流式句柄（不影响非流式在飞识别）
  void asrCancelStreaming();
  resetSession();
  if (source) void asrStopListening(source);
  // 会话被丢弃（路由/抽屉/TTS/触摸模式等门控打断）→ auto 触发标志必须复位，
  // 否则 autoTriggered 卡死 true → 能量监测永不触发（切界面后 auto_listen 失效）
  if (source === "auto") runtime.autoTriggered = false;
}

/** 同步录音 + 能量监测状态到最新可用性（任一 watch 触发时调用） */
export function updateAsrAvailability(): void {
  // 监测启停不查显示锁（canStartAsr({ ignoreLock: true })）：锁只挡"触发录音"——
  // 识别完成后锁一设监测就停、锁过期无人复活，auto_listen 永久死锁
  const wantMonitor =
    canStartAsr({ ignoreLock: true }) &&
    (runtime.asrStore?.settings.auto_listen ?? false) &&
    autoListenActive.value;
  if (wantMonitor) {
    startEnergyMonitor();
  } else {
    // 不可用 → 拆掉在飞录音 + 停能量监测。
    // 仅 recording 丢弃；recognizing 是收尾中（云端在飞识别不取消，§4 不变量），
    // 让 handleResult() 自然处理结果——否则关闭 auto_listen 会掐断正在识别的会话丢话。
    if (phase.value === "recording") {
      // 诊断：丢弃会话是"录音意外停止"的最可能路径，暴露触发原因
      asrLog().info("updateAsrAvailability 丢弃会话", {
        phase: phase.value,
        activeSource: activeSource.value,
        autoListen: runtime.asrStore?.settings.auto_listen,
      });
      discardRecording();
    }
    stopEnergyMonitor();
  }
}

/** GameDialog / ChatInput（桌宠）调用：模式开时切换功能开关（暂停/恢复自动监听）。
 *  只动运行态 autoListenActive，不改 auto_listen 模式设置（无 save）。 */
export function toggleAutoListenFunction() {
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

/** VAD 检测到一轮说话结束（turn_candidate / turn_sealed）→ 结束 auto 会话 */
export async function onVadTurnEnd() {
  asrLog().info(`VAD turn 事件, activeSource=${activeSource.value}, phase=${phase.value}`);
  if (activeSource.value !== "auto") return;
  if (phase.value === "recording") {
    stop();
  }
}

// ── 会话生命周期 ────────────────────────────────────────────
export async function start(source: AsrSource) {
  // §1 全 12 项门控；手动模式（button）跳过显示锁（总开关一律生效）
  if (!canStartAsr({ forManual: source === "button" })) {
    // 诊断：静默拒绝会让按钮"按下无反应"，暴露拒绝原因
    asrLog().info("start 被门控拒绝", {
      source,
      phase: phase.value,
      status: runtime.gameStore?.currentStatus,
      command: runtime.gameStore?.command,
      loadingComplete: runtime.gameStore?.loadingComplete,
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
  runtime.asrStore?.setMicState("recording");
  try {
    // 拼接基准：录音开始时的输入框内容（仅按钮源可拼接，auto 统一处理）
    if (source === "button") {
      runtime.baseText = runtime.inputBridge?.getText() ?? "";
    }
    // 流式：先建 WebSocket（互斥由后端 stream 检查 + start_listening 的 active 检查双层保证）。
    // llama-asr 结果流式不建 WS（stop 时整段上传），仅 qwen WS 真流式走这里
    if (isStreamEnabled() && !isLlamaStream()) {
      await asrStartStreaming({
        providerId: runtime.asrStore?.settings.active_provider ?? "qwen-asr",
        languageHint: null,
      });
    }
    runtime.stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        sampleRate: 16000,
        channelCount: 1,
        echoCancellation: true,
        noiseSuppression: true,
      },
    });
    // 竞态防护：await 挂起期间会话可能已被 stop()/discardRecording() 结束
    // （PTT 快速松开、mic 双击）——phase 已非 recording，不能继续建链，
    // 否则产生无法停止的孤儿录音（麦克风常开、pcmBuffer 无限增长）
    if (phase.value !== "recording") {
      runtime.stream.getTracks().forEach((t) => t.stop());
      // 流式会话：仅当会话已被丢弃（phase 已 idle——blur/discard/resetSession 路径）
      // 才需要 cancel 兜底——recognizing 时 stop() 已由 doStreamFinish 负责 WS 关闭，
      // 再 cancel 会同源双关产生 Canceled 噪音
      if (phase.value !== "recognizing") {
        void asrCancelStreaming();
      }
      runtime.stream = null;
      return;
    }
    runtime.audioCtx = new AudioContext({ sampleRate: 16000 });
    const src = runtime.audioCtx.createMediaStreamSource(runtime.stream);
    runtime.processor = runtime.audioCtx.createScriptProcessor(1024, 1, 1);
    src.connect(runtime.processor);
    // 输出接零增益节点而非 destination，避免把采集流回放
    const silence = runtime.audioCtx.createGain();
    silence.gain.value = 0;
    runtime.processor.connect(silence);
    silence.connect(runtime.audioCtx.destination);
    runtime.processor.onaudioprocess = (e) => {
      // teardown 后已排队的最后一次回调（≤1024 samples ≈ 64ms）可能在
      // 下个会话已启动后触发——旧音频不得混入新会话，按 phase 守卫
      if (phase.value !== "recording") return;
      const data = e.inputBuffer.getChannelData(0);
      runtime.pcmBuffer.push(...data);
      if (source === "auto") {
        runtime.vadPending.push(...data);
        // 上限保护：串行速率低于产生速率时丢弃最旧（8192 块 × 512 samples
        // ≈ 4 分钟音频，VAD 端点检测只需要最近的音频）
        if (runtime.vadPending.length > 8192 * 512) {
          runtime.vadPending.splice(0, runtime.vadPending.length - 8192 * 512);
        }
        feedVad();
      }
      if (isStreamEnabled() && !isLlamaStream()) {
        runtime.streamPending.push(...data);
        // 与 vadPending 同思路的上限保护（8192 块 ≈ 4 分钟音频）
        if (runtime.streamPending.length > 8192 * 512) {
          runtime.streamPending.splice(0, runtime.streamPending.length - 8192 * 512);
        }
        feedStream();
      }
      // 录音硬上限（1 分钟）：达到自动停止。放回调末尾——stop() 会取走
      // pcmBuffer 合成 WAV，此前的数据完整保留；VAD/流式块已在此前送完，
      // 不再残留
      if (runtime.pcmBuffer.length >= MAX_RECORD_SAMPLES) {
        stop();
      }
    };
    await asrStartListening(source);
    // 竞态兜底：await 挂起期间会话已被 stop()/discardRecording() 结束
    // （PTT 快速长按松开、mic 连点）——若后端先处理 stop（active=None）再处理本
    // start，active_source 会残留 Some(source)，后续所有会话 SessionBusy 永久卡死
    // 且前端无恢复路径。asrStopListening 幂等（无 active 会话时 Canceled 静默）
    if (phase.value !== "recording") {
      void asrStopListening(source).catch(() => {});
    }
  } catch (err: unknown) {
    const name = (err as { name?: string }).name;
    asrLog().warn("start failed:", err);
    if (name === "NotAllowedError" || name === "NotReadableError") {
      runtime.asrStore?.setMicState("denied");
      runtime.asrStore?.onError("ASR_MIC_DENIED");
    } else {
      runtime.asrStore?.onError(parseAsrError(err).code || String(err));
    }
    // 流式 WebSocket 可能已建立（getUserMedia / startListening 失败路径）：
    // 必须清理，否则后端句柄残留 → 下次启动 SessionBusy
    void asrCancelStreaming();
    // 后端 active_source 同样兜底清除（H3：start 在途被打断可能已置位）
    void asrStopListening(source).catch(() => {});
    resetSession();
    throw err;
  }
}

/** 手动结束（mic 按钮 / 快捷键松开 / VAD turn 结束）：停止 → 识别 → 处理 */
export function stop() {
  if (phase.value !== "recording") return;
  const source = activeSource.value;
  if (!source) return;
  phase.value = "recognizing";
  // 先拿走 PCM 再拆录音链路（teardownRecorder 会清空 pcmBuffer）
  const captured = runtime.pcmBuffer;
  teardownRecorder();
  void asrStopListening(source).catch(() => {
    /* 后端无 active 会话时的 Canceled 语义良性，静默 */
  });
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
    handleResult(result.text, source);
  } catch (err) {
    asrLog("[ASR/stream]").error("收尾失败:", err);
    // 错误链路打通：设置页状态面板 + mic 按钮可感知识别失败（架构 A）
    runtime.asrStore?.onError(parseAsrError(err).code || String(err));
    resetSession();
    if (source === "auto") {
      runtime.autoTriggered = false;
      updateAsrAvailability();
    }
  }
}

/** 把录音 PCM 合成 WAV 送识别，成功后 handleResult()。
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
        runtime.autoTriggered = false;
        updateAsrAvailability();
      }
      return;
    }
    const providerId = runtime.asrStore?.settings.active_provider ?? "qwen-asr";
    const result = isLlamaStream()
      ? await asrRecognizeWavStream({ providerId, wavBytes: Array.from(wav) })
      : await asrRecognizeWav({ providerId, wavBytes: Array.from(wav), languageHint: null });
    handleResult(result.text, source);
  } catch (err) {
    asrLog().error("recognize failed:", err);
    // 错误链路打通：设置页状态面板 + mic 按钮可感知识别失败（架构 A）
    runtime.asrStore?.onError(parseAsrError(err).code || String(err));
    resetSession();
    if (source === "auto") {
      runtime.autoTriggered = false;
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
export function handleResult(text: string, source: AsrSource) {
  // 识别请求在飞行中 AI 可能从 input 进入 thinking/responding/presenting
  // 返回时 currentStatus 已变 → 识别结果丢弃（不填入 / 不发送 / 不入队）。
  // voicePlaying：手动模式点击继续后 TTS 还在播，在飞识别（误录的 AI 语音）
  // 返回时同样丢弃。!chatActive：识别期间已切界面/打开设置抽屉 → 结果丢弃
  // （用户方案：没说完不发送，回来点 mic 重新启用）。
  // runningScript.choices：录音期间剧本引擎弹出选择分支（status 仍 input）→
  // 结果丢弃，否则 auto_send 的 script_submit_input 会被后端拒绝（与 canStartAsr
  // 第 8 项同一判定，这里补的是"在飞期间才弹出"的窗口期）。
  // 触摸模式/移动端菜单/加载动画同理（与 canStartAsr 第 4/5/9 项对齐——
  // 这些状态在识别在飞期间也可能翻转）。
  const script = (runtime.gameStore as unknown as { runningScript?: { choices?: unknown[] } })
    ?.runningScript;
  if (
    !runtime.gameStore ||
    runtime.gameStore.currentStatus !== "input" ||
    runtime.gameStore.command === "touch" ||
    runtime.mobileMenuOpen ||
    !runtime.gameStore.loadingComplete ||
    voicePlaying.value ||
    !chatActive.value ||
    (script && Array.isArray(script.choices) && script.choices.length > 0)
  ) {
    asrLog().info(
      `handle drop: status=${runtime.gameStore?.currentStatus}, chatActive=${chatActive.value}, ` +
        `choices=${script?.choices?.length ?? 0}, text="${text.slice(0, 30)}"`
    );
    resetSession();
    if (source === "auto") {
      runtime.autoTriggered = false;
      updateAsrAvailability();
    }
    return;
  }
  // 空识别结果（云端未识别出内容）：静默复位 + 重启监听，不 dispatch / 不发送——
  // 否则 fill_only 空串覆盖输入框（清空用户草稿），auto_send 空消息报后端"消息内容不能为空"。
  if (!text.trim()) {
    asrLog().info("handle: 空识别结果，复位会话并重启监听");
    resetSession();
    if (source === "auto") {
      runtime.autoTriggered = false;
      updateAsrAvailability();
    }
    return;
  }
  const mode = runtime.asrStore?.settings.send_mode ?? "fill_only";
  // 识别结果有效到达 = 识别服务真正在工作：清除此前残留的错误，
  // 设置页状态面板转绿（失败只写不清会让"接上服务后仍红"）
  runtime.asrStore?.clearError();
  // 拼接只对手动录音（button 源）+ fill_only 生效：识别结果追加到录音开始时的
  // 输入框内容（baseText）之后，持续录入不覆盖。auto 源与 auto_send 不拼接
  // （auto_send 只发送识别内容本身，不做与已有内容的衔接）。
  const full = source === "button" && mode === "fill_only" ? runtime.baseText + text : text;
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
    runtime.autoTriggered = false;
    updateAsrAvailability();
  }
}

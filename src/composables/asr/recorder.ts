import { activeSource, phase, runtime } from "./state";
import { asrLog } from "./log";
import { asrStreamAudioChunk, asrVadProcessChunk } from "@/api/services/asr";

/** 拆除录音链路（不触发 recognize） */
export function teardownRecorder() {
  try {
    runtime.processor?.disconnect();
  } catch {
    /* ignore */
  }
  runtime.processor = null;
  void runtime.audioCtx?.close().catch(() => {});
  runtime.audioCtx = null;
  runtime.stream?.getTracks().forEach((t) => t.stop());
  runtime.stream = null;
  runtime.pcmBuffer = [];
  runtime.vadPending = [];
  streamPending = [];
  vadSentFrames = 0;
  if (runtime.asrStore) runtime.asrStore.setMicState("idle");
}

// ── VAD 流（auto 模式）：每 512 samples（30ms @ 16k）喂后端 ──
// 严格串行单飞：一块 invoke 完成才发下一块。Silero 的 h/c 隐状态依赖
// 顺序输入——并发 fire-and-forget 会导致后端锁等待乱序，prob 结果无意义
// （表现：VAD 永不触发 SpeechStarted / TurnCandidate）。
let vadSending = false;
/** 诊断：已发送的 VAD 块数（用于降频日志） */
let vadSentFrames = 0;
export function feedVad() {
  if (!runtime.asrStore || phase.value !== "recording" || activeSource.value !== "auto") return;
  if (vadSending || runtime.vadPending.length < 512) return;
  const block = runtime.vadPending.splice(0, 512);
  vadSending = true;
  // 诊断日志：前 10 块 + 每秒 1 条（33 块），确认 VAD 流在走
  if (vadSentFrames < 10 || vadSentFrames % 33 === 0) {
    asrLog("[ASR/VAD]").info(`feedVad #${vadSentFrames} 发送 ${block.length} samples`);
  }
  vadSentFrames++;
  asrVadProcessChunk(block)
    .catch((e) => {
      // VAD 失败不阻塞录音，但错误不能静默——暴露给调试者
      asrLog("[ASR/VAD]").warn("feedVad 失败:", e);
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
export function feedStream() {
  if (!runtime.asrStore || phase.value !== "recording") return;
  if (streamSending || streamPending.length < 512) return;
  const block = streamPending.splice(0, 512);
  streamSending = true;
  asrStreamAudioChunk(block)
    .catch((e) => asrLog("[ASR/stream]").warn("发送音频块失败:", e))
    .finally(() => {
      streamSending = false;
      feedStream();
    });
}

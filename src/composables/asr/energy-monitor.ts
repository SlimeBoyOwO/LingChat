import { ENERGY_WARMUP_MS, phase, runtime } from "./state";
import { canStartAsr, chatActive } from "./gates";
import { asrLog } from "./log";

// ── 能量监测（auto_listen 常开，RMS 超阈值触发 auto 会话） ──
/** getUserMedia 挂起标记：resolve 前再次 startEnergyMonitor 直接拒绝（防并发双建链
 *  → 被覆盖实例的 ctx/stream 永不关闭，麦克风常亮 + AudioContext 泄漏） */
let energyMonPending = false;
/** 监测代标记：stopEnergyMonitor 递增，迟到的 getUserMedia resolve 据此丢弃（H2） */
let energyMonGeneration = 0;
/** auto 触发失败冷却：权限拒绝/设备错误后 N ms 内不重试（防 RMS 每帧重试刷屏，M1） */
let autoRetryCooldownUntil = 0;
/** auto 会话启动回调：由 session.ts 模块加载时注入（模块化防循环——
 *  energy-monitor 不反向依赖 session） */
let autoStartHandler: ((source: "auto") => Promise<unknown>) | null = null;

/** session.ts 注册 auto 触发启动入口（start('auto')） */
export function setAutoStartHandler(fn: (source: "auto") => Promise<unknown>): void {
  autoStartHandler = fn;
}

export function startEnergyMonitor() {
  if (runtime.energyMon || energyMonPending) return;
  // §1 全 12 项 + auto_listen 设置：任何一项不满足则不开
  if (!runtime.asrStore?.settings.auto_listen) return;
  if (!canStartAsr()) return;
  asrLog().info("startEnergyMonitor 启动 (auto_listen=on, canStartAsr=true)");
  energyMonPending = true;
  const gen = energyMonGeneration;
  navigator.mediaDevices
    .getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } })
    .then((s) => {
      energyMonPending = false;
      // 挂起期间被 stop（TTS/切界面等，gen 已变）或完整门控失效（含 TTS 播放中
      // 的 voicePlaying）→ 关闭新流丢弃，不建监测（H2：TTS 播放期麦克风不得打开）
      if (
        gen !== energyMonGeneration ||
        !runtime.asrStore?.settings.auto_listen ||
        !chatActive.value ||
        !canStartAsr()
      ) {
        asrLog().info("startEnergyMonitor 启动后条件失效，关闭 stream");
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
      const warmupUntil =
        Date.now() + (runtime.asrStore?.settings.energy_warmup_ms ?? ENERGY_WARMUP_MS);
      const tick = () => {
        if (!runtime.asrStore?.settings.auto_listen || !chatActive.value) {
          stopEnergyMonitor();
          return;
        }
        if (!runtime.energyMon) return;
        if (Date.now() < warmupUntil) {
          runtime.energyMon.raf = requestAnimationFrame(tick);
          return;
        }
        // 触发失败冷却中：跳过本帧（M1：权限拒绝后不每帧重试刷屏）
        if (Date.now() < autoRetryCooldownUntil) {
          runtime.energyMon.raf = requestAnimationFrame(tick);
          return;
        }
        analyser.getByteFrequencyData(buf);
        // RMS 归一化：byte 0-255 → 0-1，阈值 0.08 约等于明显人声能量
        let sum = 0;
        for (let i = 0; i < buf.length; i++) sum += buf[i] * buf[i];
        const rms = Math.sqrt(sum / buf.length) / 128;
        if (rms > 0.08 && phase.value === "idle" && !runtime.autoTriggered) {
          // 二次校验：AI 可能在本帧之间从 input 进入 thinking，RMS 触发时已不可用
          if (!canStartAsr()) {
            runtime.energyMon.raf = requestAnimationFrame(tick);
            return;
          }
          asrLog().info(`energy trigger: rms=${rms.toFixed(3)} > 0.08, start('auto')`);
          runtime.autoTriggered = true;
          const startFn = autoStartHandler;
          if (startFn) {
            void startFn("auto").catch((err) => {
              asrLog().warn("start(auto) failed, reset autoTriggered:", err);
              runtime.autoTriggered = false;
              // 权限拒绝/设备错误：冷却 5s 再重试（否则 RMS 每帧触发 → onError 刷屏）
              autoRetryCooldownUntil = Date.now() + 5000;
            });
          }
          return;
        }
        runtime.energyMon.raf = requestAnimationFrame(tick);
      };
      runtime.energyMon = { ctx, raf: requestAnimationFrame(tick), stream: s };
      asrLog().info("startEnergyMonitor 已建立 analyser, tick loop 开始");
    })
    .catch((err) => {
      energyMonPending = false;
      asrLog().warn("startEnergyMonitor getUserMedia 失败:", err);
      /* mic 不可用：能量监测静默降级 */
    });
}

export function stopEnergyMonitor() {
  // 递增代标记：挂起中的 getUserMedia resolve 后检测到代不符 → 关闭新流丢弃
  energyMonGeneration++;
  if (!runtime.energyMon) return;
  cancelAnimationFrame(runtime.energyMon.raf);
  void runtime.energyMon.ctx.close().catch(() => {});
  runtime.energyMon.stream.getTracks().forEach((t) => t.stop());
  runtime.energyMon = null;
}

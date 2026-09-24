/**
 * 统一帧循环调度器
 *
 * 背景：项目中的视觉效果（星光、雨雪、粒子、光标特效、频谱等）此前各自持有
 * 一个 requestAnimationFrame 自调度循环，并各自处理（或干脆没处理）页面不可见
 * 时的暂停。多个特效同时叠加时会并存多个 rAF 回调与多份可见性监听，且在窗口
 * 失焦/最小化时仍在逐帧计算。
 *
 * 本模块把这类无限循环收敛到一个共享调度器：
 * - 单个 rAF 驱动全部订阅者（订阅者只写「每帧做什么」，不再自己重新调度）
 * - 页面隐藏（document.hidden）时整个循环停摆；窗口失焦时默认也停摆
 * - 可选 per-订阅者帧率上限（例如背景星光 30fps、前景粒子 60fps）
 * - 使用「虚拟时钟」：暂停期间时间不推进，恢复后第一帧的 delta 仍是一帧的量级，
 *   因此粒子/物理类动画不会因为长时间暂停而瞬移
 *
 * 用法：
 * ```ts
 * const loop = startFrameLoop((now, delta) => {
 *   // now: 虚拟时间（毫秒，暂停期间不推进）；delta: 距上次回调的毫秒数
 *   update(delta);
 * });
 * onBeforeUnmount(() => loop.stop());
 * ```
 */

export type FrameCallback = (now: number, delta: number) => void;

export interface FrameLoopOptions {
  /** 该订阅者的帧率上限（<=0 或未设置表示跟随全局刷新率） */
  fps?: number;
  /**
   * 窗口失焦时是否也暂停（默认 true）。
   * 设为 false 用于「失焦也必须继续跑」的场景；页面隐藏时始终暂停。
   */
  pauseOnBlur?: boolean;
}

export interface FrameLoopHandle {
  /** 停止该订阅者（幂等）；最后一个订阅者停止后共享循环自动停摆 */
  stop(): void;
  /** 动态调整该订阅者的帧率上限（<=0 表示取消上限） */
  setFps(fps: number): void;
}

interface Subscriber {
  callback: FrameCallback;
  fps: number;
  pauseOnBlur: boolean;
  /** 上次回调时的虚拟时间（0 表示尚未回调过） */
  lastVirtual: number;
}

const subscribers = new Set<Subscriber>();

let rafId: number | null = null;
/** 虚拟时间：仅在循环实际运行时推进，暂停期间冻结 */
let virtualNow = 0;
/** 上一真实帧时间戳（performance.now 同源，rAF 回调参数） */
let lastRealNow = 0;
let listenersAttached = false;
let pageHidden = false;
let windowBlurred = false;

function isDocumentHidden(): boolean {
  return typeof document !== "undefined" && document.hidden;
}

/** 当前是否至少有一个订阅者需要继续跑（失焦时仍运行的订阅者） */
function hasBlurResistantSubscriber(): boolean {
  for (const subscriber of subscribers) {
    if (!subscriber.pauseOnBlur) return true;
  }
  return false;
}

function shouldRun(): boolean {
  if (pageHidden) return false;
  if (windowBlurred && !hasBlurResistantSubscriber()) return false;
  return true;
}

function schedule() {
  if (rafId !== null || !subscribers.size || !shouldRun()) return;
  rafId = requestAnimationFrame(tick);
}

function cancel() {
  if (rafId === null) return;
  cancelAnimationFrame(rafId);
  rafId = null;
}

function tick(realNow: number) {
  rafId = null;
  const frameDelta = lastRealNow ? realNow - lastRealNow : 0;
  lastRealNow = realNow;
  virtualNow += frameDelta;

  // 订阅者回调中可能 stop()，遍历副本避免迭代期间修改集合
  for (const subscriber of [...subscribers]) {
    if (!subscribers.has(subscriber)) continue;
    if (pageHidden) break;
    if (windowBlurred && subscriber.pauseOnBlur) continue;

    const previous = subscriber.lastVirtual;
    const elapsed = previous ? virtualNow - previous : frameDelta;
    if (subscriber.fps > 0) {
      const interval = 1000 / subscriber.fps;
      // 1ms 容差：抵消 rAF 时间戳的浮点抖动
      if (elapsed < interval - 1) continue;
      // 余数结转：不把「超出上限的这一帧」的余量丢掉，否则 144Hz 屏上 60fps 上限
      // 会退化成「每 3 帧跑一次」≈ 48fps（固定步长模拟会整体变慢）
      subscriber.lastVirtual = virtualNow - (elapsed % interval);
    } else {
      subscriber.lastVirtual = virtualNow;
    }
    try {
      subscriber.callback(virtualNow, elapsed);
    } catch (error) {
      console.warn("[frame-scheduler] 订阅者回调异常，已跳过该帧", error);
    }
  }

  schedule();
}

function handleVisibilityChange() {
  pageHidden = isDocumentHidden();
  if (pageHidden) {
    cancel();
    return;
  }
  resumeClock();
  schedule();
}

function handleBlur() {
  windowBlurred = true;
  if (!shouldRun()) cancel();
}

function handleFocus() {
  windowBlurred = false;
  resumeClock();
  schedule();
}

/** 恢复运行前重置基准，避免把暂停时长算进下一帧的 delta */
function resumeClock() {
  lastRealNow = 0;
}

function attachListeners() {
  if (listenersAttached) return;
  listenersAttached = true;
  pageHidden = isDocumentHidden();
  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", handleVisibilityChange);
  }
  if (typeof window !== "undefined") {
    // 与 Live2D 舞台一致：窗口失焦即暂停装饰性动画，恢复聚焦再继续
    window.addEventListener("blur", handleBlur);
    window.addEventListener("focus", handleFocus);
  }
}

/**
 * 启动一个共享帧循环。
 * - stop()：停止该订阅者（可安全重复调用）；全部停止后循环自动停摆
 * - setFps()：动态调整该订阅者的帧率上限（<=0 表示取消上限）
 */
export function startFrameLoop(
  callback: FrameCallback,
  options: FrameLoopOptions = {},
): FrameLoopHandle {
  attachListeners();
  const subscriber: Subscriber = {
    callback,
    fps: options.fps ?? 0,
    pauseOnBlur: options.pauseOnBlur ?? true,
    lastVirtual: 0,
  };
  subscribers.add(subscriber);
  schedule();

  return {
    stop() {
      if (!subscribers.delete(subscriber)) return;
      if (!subscribers.size) {
        cancel();
        // 只重置真实时间基准（避免把空档时长算进下一帧 delta），
        // 虚拟时钟保持单调：否则「最后一个订阅者停止后重新启动」会让依赖绝对相位的
        // 动画（如方向渐变、正弦闪烁）跳回起点
        lastRealNow = 0;
      }
    },
    setFps(fps: number) {
      subscriber.fps = fps > 0 ? fps : 0;
    },
  };
}

/** 当前订阅者数量（调试用） */
export function frameLoopCount(): number {
  return subscribers.size;
}

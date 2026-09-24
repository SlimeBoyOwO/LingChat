/**
 * 音频频谱采集（Web Audio API）
 *
 * 把应用内正在播放的 <audio>（背景音乐 / 环境音）接到同一个 AnalyserNode，
 * 供 SpectrumVisualizer 绘制实时频谱。
 *
 * 三条不能碰的红线（改动前务必读完）：
 *
 * 1. **只接管显式注册、且以 CORS 模式加载的元素**（crossorigin="anonymous"）。
 *    跨源媒体未经 CORS 就接进 MediaElementAudioSourceNode 会被浏览器强制静音
 *    ——所以本模块对不满足条件的元素一律不动，宁可少画几根柱子，也不能把声音弄哑。
 *    注册方见 AudioAcrossFade.vue / AmbientLoopPlayer.vue。
 *    另外自带一层兜底：CORS 加载失败的元素会退回非 CORS 加载（见 onElementError），
 *    保证"最坏情况也只是没有频谱，不会没有声音"。
 *
 * 2. **只在 AudioContext 处于 running 时接入**。接入后元素的音频改走 Web Audio 图
 *    （仍连到 destination，音量/暂停/播放速率/循环都不受影响），因此这个 AudioContext
 *    一旦建立就不能关闭；被系统挂起时靠 watchdog 在有用户手势/页面可见时恢复。
 *
 * 3. **功能关闭时完全不创建 AudioContext**（默认关闭）。未开启该功能的环境里，
 *    本模块不存在于任何播放链路上。
 */
import { ref } from "vue";

/** FFT 窗口：1024 → 512 个频点，画 20~40 根柱子足够，开销可忽略 */
const FFT_SIZE = 1024;

/** 功能开关状态（由设置项 audio.spectrumEnabled 驱动，供绘制侧读取） */
export const spectrumEnabled = ref(false);
/** 是否真的采到了音频（有元素接入且上下文在跑），用于界面提示 */
export const spectrumActive = ref(false);

let ctx: AudioContext | null = null;
let analyser: AnalyserNode | null = null;
/** 已接入的元素 → 源节点（接入不可逆，关闭功能后保留，避免把声音掐断） */
const tapped = new Map<HTMLMediaElement, MediaElementAudioSourceNode>();
/** 已注册的元素（可能因尚未加载完成而等待接入） */
const registered = new Set<HTMLMediaElement>();
let watchdogInstalled = false;

/** 当前环境是否支持 Web Audio（不支持时频谱入口整体不显示） */
export function isSpectrumSupported(): boolean {
  if (typeof window === "undefined") return false;
  return !!(
    window.AudioContext ||
    (window as unknown as { webkitAudioContext?: unknown }).webkitAudioContext
  );
}

/** 取分析节点；未开启、上下文没在跑时返回 null（绘制侧据此走静息动画） */
export function getSpectrumAnalyser(): AnalyserNode | null {
  return spectrumEnabled.value && ctx && ctx.state === "running" ? analyser : null;
}

/** 采样率，用于把频点换算成 Hz（对数分频用） */
export function getSpectrumSampleRate(): number {
  return ctx?.sampleRate || 44100;
}

function ensureContext(): AudioContext | null {
  if (ctx) return ctx;
  const Ctor =
    window.AudioContext ||
    (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Ctor) return null;
  try {
    // 走 window 上的构造器：audioOutputManager 包装过它，
    // 这样新上下文会自动跟随"音频输出设备"设置切换（桌面端）。
    ctx = new Ctor();
  } catch (e) {
    console.warn("[spectrum] 创建 AudioContext 失败，频谱不可用:", e);
    return null;
  }
  analyser = ctx.createAnalyser();
  analyser.fftSize = FFT_SIZE;
  analyser.smoothingTimeConstant = 0.72;
  // 动态范围（默认 -100/-30）：整体略抬一点，小柱子上更活泼，又不至于一直顶格
  analyser.minDecibels = -95;
  analyser.maxDecibels = -25;
  // 直通输出：被接入的元素仍要能正常发声
  analyser.connect(ctx.destination);
  ctx.addEventListener("statechange", () => {
    if (!ctx) return;
    if (ctx.state === "running") {
      attachAll();
    } else if (spectrumEnabled.value && ctx.state === "suspended") {
      // 挂起时已接入的元素会静音，交由 watchdog 在有用户手势时恢复
      spectrumActive.value = false;
    }
  });
  return ctx;
}

/** 判断元素此刻能否安全接入 */
function canAttach(el: HTMLMediaElement): boolean {
  if (!el || tapped.has(el)) return false;
  if (!spectrumEnabled.value || !ctx || ctx.state !== "running" || !analyser) return false;
  // 未走 CORS 加载的媒体接进去会被静音，直接放弃
  if (el.crossOrigin !== "anonymous") return false;
  // 还没加载成功（或已失败）：等 loadeddata/play 再补挂
  if (el.error || !el.currentSrc) return false;
  return true;
}

function attach(el: HTMLMediaElement): boolean {
  if (!canAttach(el)) return false;
  try {
    const node = ctx!.createMediaElementSource(el);
    node.connect(analyser!);
    tapped.set(el, node);
    spectrumActive.value = true;
    return true;
  } catch (e) {
    console.warn("[spectrum] 音频元素接入失败（该元素不参与频谱）:", e);
    return false;
  }
}

function attachAll(): void {
  if (!spectrumEnabled.value || !ctx || ctx.state !== "running") return;
  registered.forEach((el) => attach(el));
  spectrumActive.value = tapped.size > 0;
}

/** 元素就绪（加载完成 / 开始播放）时补挂 */
const onElementReady = () => {
  if (spectrumEnabled.value) attachAll();
};

/** 元素"是否正在播"，用于 CORS 兜底后决定要不要续播 */
const playingFlags = new WeakMap<HTMLMediaElement, boolean>();

/**
 * CORS 加载失败的兜底：某些音源（或不支持 CORS 的自定义协议）在
 * crossorigin="anonymous" 下会直接加载失败、彻底没声。
 * 这里退回非 CORS 加载——声音优先，该元素从此不参与频谱。
 * 每个元素只兜底一次，且已接入音频图的元素绝不动（断开/改载会让它变哑）。
 */
function onElementError(e: Event) {
  const el = e.currentTarget as HTMLAudioElement | null;
  if (!el || !el.crossOrigin || tapped.has(el)) return;
  el.removeEventListener("error", onElementError);
  el.removeAttribute("crossorigin");
  const wasPlaying = playingFlags.get(el) ?? false;
  el.src = el.src; // 重新触发媒体加载算法
  el.load();
  if (wasPlaying) el.play().catch(() => {});
  console.warn("[spectrum] 媒体 CORS 加载失败，已退回普通加载（该音源不参与频谱）");
}

const onElementPlay = (e: Event) => {
  playingFlags.set(e.currentTarget as HTMLMediaElement, true);
};

const onElementPause = (e: Event) => {
  playingFlags.set(e.currentTarget as HTMLMediaElement, false);
};

/**
 * 注册一个待采集的音频元素。
 * 元素需带 crossorigin="anonymous"（见文件头红线 1）；不满足时静默跳过。
 */
export function registerSpectrumSource(el: HTMLMediaElement | null | undefined): void {
  if (!el || registered.has(el)) return;
  registered.add(el);
  el.addEventListener("loadeddata", onElementReady);
  el.addEventListener("play", onElementReady);
  el.addEventListener("play", onElementPlay);
  el.addEventListener("pause", onElementPause);
  el.addEventListener("error", onElementError);
  attach(el);
}

/** 注销（组件卸载时调用）：断开连接，让元素可被回收 */
export function unregisterSpectrumSource(el: HTMLMediaElement | null | undefined): void {
  if (!el) return;
  registered.delete(el);
  el.removeEventListener("loadeddata", onElementReady);
  el.removeEventListener("play", onElementReady);
  el.removeEventListener("play", onElementPlay);
  el.removeEventListener("pause", onElementPause);
  el.removeEventListener("error", onElementError);
  const node = tapped.get(el);
  if (node) {
    try {
      node.disconnect();
    } catch {
      /* 已断开 */
    }
    tapped.delete(el);
  }
  spectrumActive.value = tapped.size > 0;
}

/**
 * 全局兜底：页面重新可见或有用户手势时，尝试唤醒上下文并补挂元素。
 * 一旦安装不再移除——它同时承担"保持音频图存活"的职责。
 */
function installWatchdog(): void {
  if (watchdogInstalled) return;
  watchdogInstalled = true;
  const kick = () => {
    if (!spectrumEnabled.value || !ctx) return;
    if (ctx.state === "running") {
      attachAll();
      return;
    }
    ctx
      .resume()
      .then(() => attachAll())
      .catch(() => {
        /* 仍缺用户手势，等下一次触发 */
      });
  };
  window.addEventListener("pointerdown", kick, true);
  window.addEventListener("keydown", kick, true);
  document.addEventListener("visibilitychange", kick);
}

/** 开关频谱采集（对应设置项 audio.spectrumEnabled） */
export function setSpectrumEnabled(next: boolean): void {
  if (next === spectrumEnabled.value) return;
  spectrumEnabled.value = next;
  // 关闭：保留已建成的音频图（断开会让正在播的元素静音），只是不再采集
  if (!next) {
    spectrumActive.value = false;
    return;
  }
  installWatchdog();
  const c = ensureContext();
  if (!c) return;
  if (c.state === "running") {
    attachAll();
    return;
  }
  c.resume()
    .then(() => attachAll())
    .catch(() => {
      /* 首次开启若不在用户手势内，等 watchdog 唤醒 */
    });
}

/**
 * 音频频谱绘制核心（与 utils/audioSpectrum.ts 的采集层配套）
 *
 * SpectrumVisualizer（/chat 右下角）与 PetSpectrumRing（/pet 头像外圈）共用：
 * 对数分频、快起慢落的平滑、静息呼吸动画、DPR 铺底，以及 mirror / bars / ring
 * 三种形态的绘制。两边的配色与形态设置同源（constants/spectrum.ts），
 * 绘制逻辑必须只有一份，否则调一次呼吸曲线就会悄悄漂移。
 *
 * 平滑状态放在实例里：每个绘制方各自 new 一个、各自跑自己的 rAF
 * （两边帧预算与可见性条件不同），每帧先 update(now) 再 draw(canvas, spec, colors)。
 */
import { getSpectrumAnalyser, getSpectrumSampleRate } from "@/utils/audioSpectrum";
import type { SpectrumStyle } from "@/constants/spectrum";

/** 默认频带数（主界面三种形态里用到的最大柱数，其余按比例合并） */
export const DEFAULT_SPECTRUM_BANDS = 48;

export interface SpectrumSpec {
  kind: SpectrumStyle;
  w: number;
  h: number;
  bars: number;
  gap: number;
  glow: number;
  /** 圆环：基准半径 / 单根柱最大长度 */
  r0: number;
  len: number;
  /** 圆环柱宽占相邻柱间距的比例，默认 0.55（留出可见的缝）；给 1 则相邻柱相接、连成一整圈 */
  barWidthRatio?: number;
  /**
   * 圆环：频带沿圆周铺满一圈时，低频与高频各占半圈，而音乐能量集中在低频，
   * 结果只有半圈跳动。开启后后半圈镜像折回，绕一圈左右对称。
   */
  foldSpectrum?: boolean;
  /**
   * 裸模式：不做任何装饰性兜底 —— 柱子没有最小长度、静默时不走呼吸动画、不画基准圆。
   * 能量为 0 就是真的什么都不显示（桌宠头像外圈的圆环用它）。
   */
  bare?: boolean;
}

export interface SpectrumColors {
  from: string;
  to: string;
}

/** 能量包络的跟随速度：每帧向当前值靠拢的比例。越小越平滑，越大越跟手 */
export interface SpectrumSmoothing {
  rise: number;
  fall: number;
}

/** 主界面沿用至今的手感：上升立刻到位、下落 0.18，跟得住快歌的鼓点 */
const DEFAULT_SMOOTHING: SpectrumSmoothing = { rise: 1, fall: 0.18 };

function roundRect(
  c: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  const rr = Math.max(0, Math.min(r, w / 2, h / 2));
  c.beginPath();
  // 原生 roundRect 更省事，缺了就走 arcTo 兜底（旧 WebKitGTK）
  const native = c as unknown as {
    roundRect?: (x: number, y: number, w: number, h: number, r: number) => void;
  };
  if (typeof native.roundRect === "function") {
    native.roundRect(x, y, w, h, rr);
    return;
  }
  c.moveTo(x + rr, y);
  c.arcTo(x + w, y, x + w, y + h, rr);
  c.arcTo(x + w, y + h, x, y + h, rr);
  c.arcTo(x, y + h, x, y, rr);
  c.arcTo(x, y, x + w, y, rr);
  c.closePath();
}

function hexToRgb(hex: string): [number, number, number] {
  const m = /^#?([\da-f]{3}|[\da-f]{6})$/i.exec((hex || "").trim());
  if (!m) return [121, 217, 255];
  let h = m[1]!;
  if (h.length === 3)
    h = h
      .split("")
      .map((ch) => ch + ch)
      .join("");
  const n = parseInt(h, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** 两色插值（圆环沿圆周做色相过渡用） */
function mixColor(from: string, to: string, t: number): string {
  const a = hexToRgb(from);
  const b = hexToRgb(to);
  const mix = (x: number, y: number) => Math.round(x + (y - x) * t);
  return `rgb(${mix(a[0], b[0])}, ${mix(a[1], b[1])}, ${mix(a[2], b[2])})`;
}

/** 画布按 DPR 铺底，小尺寸下也不糊 */
function prepare(canvas: HTMLCanvasElement, spec: SpectrumSpec): CanvasRenderingContext2D | null {
  const c = canvas.getContext("2d");
  if (!c) return null;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const bw = Math.round(spec.w * dpr);
  const bh = Math.round(spec.h * dpr);
  if (canvas.width !== bw || canvas.height !== bh) {
    canvas.width = bw;
    canvas.height = bh;
    canvas.style.width = `${spec.w}px`;
    canvas.style.height = `${spec.h}px`;
  }
  c.setTransform(dpr, 0, 0, dpr, 0, 0);
  c.clearRect(0, 0, spec.w, spec.h);
  return c;
}

export class SpectrumRenderer {
  /** 频带数：由实例各自决定。柱数多于频带数时相邻柱会取到同一条频带、
      看起来就是"变宽"而不是"变密"，所以想要更密的刻度要连这里一起加 */
  private readonly bands: number;
  /** 包络跟随速度 */
  private readonly smoothing: SpectrumSmoothing;
  /** 每个频带的平滑能量（0~1） */
  private levels: Float32Array;
  private freqData = new Uint8Array(0);
  private bandEdges = new Int32Array(0);
  private bandSampleRate = 0;
  /** 0 = 实时数据，1 = 静息呼吸动画（两者之间平滑过渡） */
  private idleMix = 0;
  private idleClock = 0;

  constructor(
    bands: number = DEFAULT_SPECTRUM_BANDS,
    smoothing: SpectrumSmoothing = DEFAULT_SMOOTHING,
  ) {
    // 下限 2：高频倾斜补偿里要除以 bands - 1
    this.bands = Math.max(2, Math.floor(bands));
    this.smoothing = smoothing;
    this.levels = new Float32Array(this.bands);
  }

  /** 按对数分频铺频带：低频窄、高频宽，听感上更均匀 */
  private ensureBands(binCount: number) {
    const sr = getSpectrumSampleRate();
    if (this.bandEdges.length === this.bands + 1 && this.bandSampleRate === sr) return;
    const nyquist = sr / 2;
    const lo = 45;
    const hi = Math.min(13000, nyquist * 0.9);
    const edges = new Int32Array(this.bands + 1);
    for (let i = 0; i <= this.bands; i++) {
      const f = lo * Math.pow(hi / lo, i / this.bands);
      edges[i] = Math.min(binCount - 1, Math.max(0, Math.round((f / nyquist) * binCount)));
    }
    // 保证每根柱子至少占一个频点
    for (let i = 1; i <= this.bands; i++) {
      if (edges[i]! <= edges[i - 1]!) edges[i] = Math.min(binCount - 1, edges[i - 1]! + 1);
    }
    this.bandEdges = edges;
    this.bandSampleRate = sr;
  }

  /** 采样一帧：读分析节点 → 铺频带 → 快起慢落 → 与静息呼吸动画混合 */
  update(now: number) {
    const a = getSpectrumAnalyser();
    let peak = 0;
    if (a) {
      if (this.freqData.length !== a.frequencyBinCount)
        this.freqData = new Uint8Array(a.frequencyBinCount);
      a.getByteFrequencyData(this.freqData);
      this.ensureBands(a.frequencyBinCount);
      for (let i = 0; i < this.bands; i++) {
        const start = this.bandEdges[i]!;
        const end = Math.max(start + 1, this.bandEdges[i + 1]!);
        let max = 0;
        for (let k = start; k < end && k < this.freqData.length; k++) {
          if (this.freqData[k]! > max) max = this.freqData[k]!;
        }
        // 高频能量天然偏弱，做一点倾斜补偿，否则只有左边几根跳
        const v = Math.min(1, (max / 255) * (1 + 0.6 * (i / (this.bands - 1))));
        // 快起慢落：两个系数都调小则整体更平滑，调大则更跟手
        const prev = this.levels[i]!;
        this.levels[i] = prev + (v - prev) * (v > prev ? this.smoothing.rise : this.smoothing.fall);
        if (this.levels[i]! > peak) peak = this.levels[i]!;
      }
    } else {
      for (let i = 0; i < this.bands; i++)
        this.levels[i] = this.levels[i]! + (0 - this.levels[i]!) * 0.12;
    }
    this.idleClock = now;
    // 长时间没有能量 → 切到呼吸动画，避免看起来像卡死
    this.idleMix += ((peak < 0.035 ? 1 : 0) - this.idleMix) * 0.06;
  }

  /** 柱序号 i 在频带轴上的位置（0~1）。fold 时后半圈镜像折回 */
  private bandPos(i: number, bars: number, fold: boolean): number {
    const p = i / bars;
    if (!fold) return p;
    return p < 0.5 ? p * 2 : (1 - p) * 2;
  }

  /** 取第 i 根柱子的显示值（实时数据与静息动画按 idleMix 混合；裸模式只取实时值） */
  private barValue(i: number, spec: SpectrumSpec): number {
    const bars = spec.bars;
    const fold = !!spec.foldSpectrum;
    const start = Math.min(this.bands - 1, Math.round(this.bandPos(i, bars, fold) * this.bands));
    const end = Math.max(start + 1, Math.round(this.bandPos(i + 1, bars, fold) * this.bands));
    let v = 0;
    let n = 0;
    for (let k = start; k < end && k < this.bands; k++) {
      v += this.levels[k]!;
      n++;
    }
    v /= Math.max(1, n);

    if (spec.bare || this.idleMix <= 0.001) return v;
    // 圆环首尾要接得上：空间相位取整数圈，接缝处才不会跳
    const p = i / bars;
    const wrap = spec.kind === "ring";
    const t = this.idleClock * 0.0016;
    const spatial = wrap ? p * Math.PI * 4 : p * 6.2;
    const idle = Math.max(
      0.03,
      0.062 + 0.045 * Math.sin(spatial - t * 1.6) + 0.02 * Math.sin(p * 2.3 + t),
    );
    return v * (1 - this.idleMix) + idle * this.idleMix;
  }

  /** 镜像 / 柱状：横向排布的胶囊柱 */
  private drawLinear(c: CanvasRenderingContext2D, spec: SpectrumSpec, colors: SpectrumColors) {
    const { from, to } = colors;
    const grad = c.createLinearGradient(0, 0, spec.w, 0);
    grad.addColorStop(0, from);
    grad.addColorStop(1, to);

    const bars = spec.bars;
    const barW = (spec.w - spec.gap * (bars - 1)) / bars;
    // 四周留出辉光余量，别让外发光被画布裁掉
    const pad = spec.glow * 0.9;
    const midY = spec.h / 2;
    const halfMax = midY - pad;
    const fullMax = spec.h - pad * 2;
    const mirror = spec.kind === "mirror";

    // 镜像形态给一条极淡的中线，上下对称关系才看得清
    if (mirror) {
      c.globalAlpha = 0.12;
      c.fillStyle = grad;
      c.fillRect(0, midY - 0.5, spec.w, 1);
    }

    c.fillStyle = grad;
    c.shadowColor = from;
    c.shadowBlur = spec.glow;
    const minLen = spec.bare ? 0 : barW * 0.62;
    const maxLen = mirror ? halfMax : fullMax;
    for (let i = 0; i < bars; i++) {
      const v = this.barValue(i, spec);
      const len = Math.max(minLen, Math.min(maxLen, v * maxLen));
      // 没有能量就不画：退化成一个点也仍会被辉光渲出一丝发丝线
      if (len < 0.5) continue;
      const x = i * (barW + spec.gap);
      c.globalAlpha = 0.34 + 0.66 * Math.min(1, v * 1.5);
      if (mirror) {
        roundRect(c, x, midY - len, barW, len * 2, barW / 2);
      } else {
        roundRect(c, x, spec.h - pad - len, barW, len, barW / 2);
      }
      c.fill();
    }
    c.globalAlpha = 1;
    c.shadowBlur = 0;
  }

  /** 圆环：绕圆周向外辐射的柱子，颜色沿圆周 from→to 过渡 */
  private drawRing(c: CanvasRenderingContext2D, spec: SpectrumSpec, colors: SpectrumColors) {
    const { from, to } = colors;
    const cx = spec.w / 2;
    const cy = spec.h / 2;
    const bars = spec.bars;
    const r0 = spec.r0;
    const barW = Math.max(1, ((2 * Math.PI * r0) / bars) * (spec.barWidthRatio ?? 0.55));
    const minLen = spec.bare ? 0 : barW * 0.7;

    // 基准圆环（裸模式下不画：那是一圈常驻的装饰，没声音时也应该什么都不显示）
    if (!spec.bare) {
      c.beginPath();
      c.arc(cx, cy, r0, 0, Math.PI * 2);
      c.strokeStyle = from;
      c.lineWidth = Math.max(0.6, barW * 0.35);
      c.globalAlpha = 0.16;
      c.stroke();
    }

    c.shadowBlur = spec.glow;
    for (let i = 0; i < bars; i++) {
      const v = this.barValue(i, spec);
      const len = Math.max(minLen, Math.min(spec.len, v * spec.len));
      // 没有能量就不画：退化成一个点也仍会被辉光渲出一丝发丝线
      if (len < 0.5) continue;
      const color = mixColor(from, to, i / bars);
      const angle = -Math.PI / 2 + (i / bars) * Math.PI * 2;
      c.save();
      c.translate(cx, cy);
      c.rotate(angle);
      c.globalAlpha = 0.34 + 0.66 * Math.min(1, v * 1.5);
      c.fillStyle = color;
      c.shadowColor = color;
      roundRect(c, -barW / 2, -(r0 + len), barW, len, barW / 2);
      c.fill();
      c.restore();
    }
    c.globalAlpha = 1;
    c.shadowBlur = 0;
  }

  draw(canvas: HTMLCanvasElement | null, spec: SpectrumSpec, colors: SpectrumColors) {
    if (!canvas) return;
    const c = prepare(canvas, spec);
    if (!c) return;
    if (spec.kind === "ring") this.drawRing(c, spec, colors);
    else this.drawLinear(c, spec, colors);
  }
}

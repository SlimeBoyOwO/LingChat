import type { Ref } from "vue";
import { useParticleField, type ParticleProps } from "./useParticleField";
import {
  BOB_AMPLITUDE,
  BOB_PERIOD,
  BREATH_AMPLITUDE,
  BREATH_PERIOD,
  MAX_BACKING_PIXELS,
  MAX_SCALE,
  MIN_SCALE,
  SPRITE_HEIGHT,
  SPRITE_WIDTH,
  WISP_COLOR,
  WISP_LAYERS,
  WISP_VARIANTS,
  type WispLayerOption,
} from "../config/fog";

const TAU = Math.PI * 2;
/** 单帧最大步长，秒。切后台再回来时不让云瞬移一大截 */
const MAX_STEP = 0.05;

function mulberry32(a: number) {
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * 预渲染一张云影精灵。
 *
 * 沿横轴铺一串大小不一的径向渐变，中间鼓、两端小，凑出不规则的轮廓 ——
 * 单个径向渐变只能得到一颗圆点，而云的关键恰恰是轮廓不规则。
 * 每个渐变的峰值都压得低，靠重叠累积出浓淡，才留得住内部的疏密变化。
 *
 * 没用 ctx.filter 做模糊：它在部分 WebKit 上不可用，而径向渐变本身
 * 就是软的，不需要再糊一次。
 */
function makeWispSprite(seed: number): HTMLCanvasElement {
  const rand = mulberry32(seed);
  const canvas = document.createElement("canvas");
  canvas.width = SPRITE_WIDTH;
  canvas.height = SPRITE_HEIGHT;
  const ctx = canvas.getContext("2d");
  if (!ctx) return canvas;

  const w = SPRITE_WIDTH;
  const h = SPRITE_HEIGHT;
  const blobs = 12 + Math.floor(rand() * 8);

  for (let i = 0; i < blobs; i++) {
    const t = i / (blobs - 1);
    // 两端的团小、中间的团大，云才有中间鼓、两侧散开的形状
    const bulge = Math.sin(Math.PI * t);
    // 半径按高度算而不是宽度：精灵是宽扁的，按宽度算会让云在纵向上
    // 溢出精灵边界，被硬生生切掉一条底边
    const r = h * (0.1 + 0.16 * bulge * (0.6 + rand() * 0.7));
    // 中心到边界的距离必须留出整个半径的余量，云的四边才都是软的
    const cx = w * (0.16 + 0.68 * t) + (rand() * 2 - 1) * w * 0.04;
    const cy = h * (0.62 - bulge * 0.14) + (rand() * 2 - 1) * h * 0.08;
    // 峰值给足：云的内部是浓的、只有边缘才软。每个团都压得很低的话，
    // 重叠也累积不出实心的芯，整朵云就只剩一片几乎透明的雾，什么也看不见
    const peak = 0.3 + rand() * 0.25;

    const grad = ctx.createRadialGradient(cx, cy, 0, cx, cy, r);
    grad.addColorStop(0, `rgba(${WISP_COLOR}, ${peak})`);
    grad.addColorStop(0.35, `rgba(${WISP_COLOR}, ${peak * 0.94})`);
    grad.addColorStop(0.7, `rgba(${WISP_COLOR}, ${peak * 0.54})`);
    grad.addColorStop(1, `rgba(${WISP_COLOR}, 0)`);
    ctx.fillStyle = grad;
    // 渐变到 r 处已经归零，不必再建路径去裁圆
    ctx.fillRect(cx - r, cy - r, r * 2, r * 2);
  }

  // 上缘压一道纵向渐隐：云和雾都是上蓬下平，顶部不该和底部一样实
  ctx.globalCompositeOperation = "destination-in";
  const mask = ctx.createLinearGradient(0, 0, 0, h);
  mask.addColorStop(0, "rgba(255, 255, 255, 0.3)");
  mask.addColorStop(0.45, "rgba(255, 255, 255, 0.85)");
  mask.addColorStop(1, "rgba(255, 255, 255, 1)");
  ctx.fillStyle = mask;
  ctx.fillRect(0, 0, w, h);
  ctx.globalCompositeOperation = "source-over";

  return canvas;
}

/** 一层的云团。用平铺数组而不是对象数组，热循环里不产生任何垃圾。 */
interface WispField {
  option: WispLayerOption;
  x: Float32Array;
  y: Float32Array;
  w: Float32Array;
  h: Float32Array;
  speed: Float32Array;
  alpha: Float32Array;
  /** 呼吸与上下浮动的相位，逐团错开才不会整片一起脉动 */
  phase: Float32Array;
  variant: Uint8Array;
  count: number;
}

/**
 * 雾的渲染场。
 *
 * 每帧只做两件事：推进横坐标，再把每团云按各自的透明度画上去。
 * 没有批绘制 —— 每团的透明度、大小、形状都不同，硬凑成同一批反而要
 * 反复切状态。云团的量级是十几团，逐团绘制完全不是瓶颈，
 * 真正的成本在它们盖住的像素上，所以配置里把数量卡得很死。
 */
class FogField {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly sprites: HTMLCanvasElement[];

  private layers: WispField[] = [];
  private intensity: number;
  private cssW = 0;
  private cssH = 0;
  private scale = 0;

  private wantRun = false;
  private running = false;
  private rafId = 0;
  private lastTs = 0;
  private elapsed = 0;
  /** 上一帧是否真的画了东西，强度归零时靠它决定要不要擦一次 */
  private painted = false;

  constructor(canvas: HTMLCanvasElement, intensity: number) {
    this.canvas = canvas;
    // 不要传 willReadFrequently：那是为反复读像素准备的，会把画布退回软件光栅
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("雾效需要 2d 画布上下文");
    this.ctx = ctx;
    this.intensity = intensity;
    this.sprites = Array.from({ length: WISP_VARIANTS }, (_, i) =>
      makeWispSprite(0x9e37 + i * 7919),
    );
  }

  setIntensity(value: number) {
    const next = Math.max(0, Math.min(2, value));
    if (next === this.intensity) return;
    this.intensity = next;
  }

  resize() {
    const host = this.canvas.parentElement;
    const cssW = Math.max(1, Math.round(host ? host.clientWidth : window.innerWidth));
    const cssH = Math.max(1, Math.round(host ? host.clientHeight : window.innerHeight));
    if (cssW < 2 || cssH < 2) return;

    const dpr = window.devicePixelRatio || 1;
    const budget = Math.sqrt(MAX_BACKING_PIXELS / (cssW * cssH));
    const scale = Math.max(MIN_SCALE, Math.min(dpr, MAX_SCALE, budget));
    const sameSize = cssW === this.cssW && cssH === this.cssH;
    if (scale === this.scale && sameSize) return;

    // 尺寸变化时按比例迁移已有的云团，而不是整片重排 ——
    // 对话框一展开就重排的话，画面上的云会整批跳一下
    if (sameSize || this.cssW < 2) {
      this.cssW = cssW;
      this.cssH = cssH;
    } else {
      const sx = cssW / this.cssW;
      const sy = cssH / this.cssH;
      this.cssW = cssW;
      this.cssH = cssH;
      for (const layer of this.layers) {
        for (let i = 0; i < layer.count; i++) {
          layer.x[i]! *= sx;
          layer.y[i]! *= sy;
          layer.w[i]! *= sx;
          layer.h[i]! *= sy;
        }
      }
    }

    this.scale = scale;
    this.canvas.width = Math.round(cssW * scale);
    this.canvas.height = Math.round(cssH * scale);
    // 改动画布尺寸会重置上下文状态，变换要重设
    this.ctx.setTransform(scale, 0, 0, scale, 0, 0);

    this.build();

    // 挂载时容器还没有尺寸的话，start 会落空，这里补一次
    if (this.wantRun && !this.running) this.start();
  }

  start() {
    this.wantRun = true;
    if (this.running || this.cssW < 2) return;
    this.running = true;
    this.lastTs = 0;
    this.rafId = requestAnimationFrame(this.frame);
  }

  stop() {
    this.wantRun = false;
    if (!this.running) return;
    this.running = false;
    cancelAnimationFrame(this.rafId);
  }

  destroy() {
    this.stop();
    this.layers = [];
    this.sprites.length = 0;
  }

  /** 按当前尺寸与强度重建云场。只在首次布场与尺寸变化时调用 */
  private build() {
    if (this.cssW < 2 || this.cssH < 2) return;

    const area = this.cssW * this.cssH;
    this.layers = WISP_LAYERS.map((option) => {
      const count = Math.min(
        option.maxCount,
        Math.max(1, Math.round(((option.perMegapixel * area) / 1e6) * this.intensity)),
      );
      const layer: WispField = {
        option,
        x: new Float32Array(count),
        y: new Float32Array(count),
        w: new Float32Array(count),
        h: new Float32Array(count),
        speed: new Float32Array(count),
        alpha: new Float32Array(count),
        phase: new Float32Array(count),
        variant: new Uint8Array(count),
        count,
      };
      for (let i = 0; i < count; i++) this.scatter(layer, i, true);
      return layer;
    });

    // 强度调到 0 时循环不再绘制，得在这里把上一帧擦掉，否则雾会冻在画布上
    this.ctx.clearRect(0, 0, this.cssW, this.cssH);
  }

  /** 布下一朵云。initial 为真时铺满整屏，否则从右边界外飘入 */
  private scatter(layer: WispField, i: number, initial: boolean) {
    const option = layer.option;
    const width =
      this.cssW *
      (option.minWidthRatio + Math.random() * (option.maxWidthRatio - option.minWidthRatio));
    const height = width * (SPRITE_HEIGHT / SPRITE_WIDTH);

    layer.w[i] = width;
    layer.h[i] = height;
    layer.x[i] = initial
      ? Math.random() * this.cssW
      : this.cssW + width * (0.5 + Math.random() * 0.6);
    layer.y[i] = this.cssH * (option.minY + Math.random() * (option.maxY - option.minY));
    layer.speed[i] = option.minSpeed + Math.random() * (option.maxSpeed - option.minSpeed);
    layer.alpha[i] = option.minAlpha + Math.random() * (option.maxAlpha - option.minAlpha);
    layer.phase[i] = Math.random();
    layer.variant[i] = Math.floor(Math.random() * WISP_VARIANTS);
  }

  private frame = (timestamp: number) => {
    if (!this.running) return;
    if (this.lastTs === 0) this.lastTs = timestamp;
    const dt = Math.min((timestamp - this.lastTs) / 1000, MAX_STEP);
    this.lastTs = timestamp;
    this.elapsed += dt;

    if (this.intensity > 0) {
      this.update(dt);
      this.draw();
      this.painted = true;
    } else if (this.painted) {
      this.ctx.clearRect(0, 0, this.cssW, this.cssH);
      this.painted = false;
    }
    this.rafId = requestAnimationFrame(this.frame);
  };

  private update(dt: number) {
    for (const layer of this.layers) {
      const count = layer.count;
      for (let i = 0; i < count; i++) {
        layer.x[i]! += layer.speed[i]! * dt;
        // 整团移出右边界就回到左边重新布一朵，形状与浓淡都换过
        if (layer.x[i]! - layer.w[i]! * 0.5 > this.cssW) this.scatter(layer, i, false);
      }
    }
  }

  private draw() {
    const ctx = this.ctx;
    ctx.clearRect(0, 0, this.cssW, this.cssH);

    for (const layer of this.layers) {
      const count = layer.count;
      for (let i = 0; i < count; i++) {
        const w = layer.w[i]!;
        const h = layer.h[i]!;
        const phase = layer.phase[i]!;
        const angle = (this.elapsed / BREATH_PERIOD + phase) * TAU;

        // 呼吸与浮动各用一条周期互质的正弦，两者的相位叠加起来不重复
        const breathe = 1 + BREATH_AMPLITUDE * Math.sin(angle);
        const bob = Math.sin((this.elapsed / BOB_PERIOD + phase) * TAU) * h * BOB_AMPLITUDE;

        ctx.globalAlpha = layer.alpha[i]! * breathe * this.intensity;
        ctx.drawImage(
          this.sprites[layer.variant[i]!]!,
          layer.x[i]! - w * 0.5,
          layer.y[i]! + bob - h * 0.5,
          w,
          h,
        );
      }
    }
    ctx.globalAlpha = 1;
  }
}

/**
 * 把雾的渲染场挂到画布上。尺寸观察与生命周期交给 useParticleField。
 */
export function useFog(canvasRef: Ref<HTMLCanvasElement | null>, props: ParticleProps) {
  useParticleField(canvasRef, props, (canvas, intensity) => new FogField(canvas, intensity));
}

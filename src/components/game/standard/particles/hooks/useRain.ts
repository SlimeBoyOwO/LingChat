import { onBeforeUnmount, onMounted, watch, type Ref } from "vue";
import {
  BOKEH,
  DROPS_PER_MEGAPIXEL,
  MAX_BACKING_PIXELS,
  MAX_DROPS,
  MAX_SCALE,
  MIN_SCALE,
  RAIN_LAYERS,
  SHUTTER_SECONDS,
  SPEED_JITTER,
  SPLASH,
  SPLASH_COLOR,
  STREAK_COLOR,
  WIND,
  type RainLayerOption,
} from "../config/rain";

const TAU = Math.PI * 2;
/** 单帧最大步长，秒。切后台再回来时不让雨滴瞬移一大截 */
const MAX_STEP = 0.05;
/** 横向环绕边距，像素。风把雨吹向一侧，靠环绕维持分布均匀 */
const WRAP_MARGIN = 120;
/** 生成时落在画布上方多少像素内，让雨滴从画面外飘入而不是凭空出现 */
const SPAWN_BAND = 160;
/** 涟漪按生命周期分批绘制，这是批次数 */
const SPLASH_STAGES = 3;
/** 同时存在的涟漪上限 */
const MAX_SPLASHES = 128;

/** 一层雨滴。用平铺数组而不是对象数组，热循环里不产生任何垃圾。 */
interface LayerField {
  option: RainLayerOption;
  x: Float32Array;
  y: Float32Array;
  z: Float32Array;
  vy: Float32Array;
  count: number;
}

interface SplashField {
  x: Float32Array;
  y: Float32Array;
  life: Float32Array;
  maxR: Float32Array;
  count: number;
}

interface BokehField {
  x: Float32Array;
  y: Float32Array;
  r: Float32Array;
  speed: Float32Array;
  alpha: Float32Array;
  count: number;
}

/**
 * 雨的渲染场。
 *
 * 性能的关键在于「按层批绘制」：同一景深的所有雨滴进同一条路径，一次 stroke 画完，
 * 全场雨丝只有六次绘制调用，且热循环里没有对象创建。为此牺牲的是逐滴的独立参数，
 * 所以每滴的速度、长度、透明度都由它所在层的深度推导，而不是各存一份。
 */
class RainField {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;

  private intensity: number;
  private cssW = 0;
  private cssH = 0;
  private scale = 0;

  private layers: LayerField[] = [];
  private splash: SplashField = {
    x: new Float32Array(0),
    y: new Float32Array(0),
    life: new Float32Array(0),
    maxR: new Float32Array(0),
    count: 0,
  };
  private bokeh: BokehField = {
    x: new Float32Array(0),
    y: new Float32Array(0),
    r: new Float32Array(0),
    speed: new Float32Array(0),
    alpha: new Float32Array(0),
    count: 0,
  };
  /** 单位空间里的径向渐变，靠逐滴的 translate/scale 复用，不逐帧重建 */
  private bokehGradient: CanvasGradient | null = null;

  private wantRun = false;
  private running = false;
  private rafId = 0;
  private lastTs = 0;
  private elapsed = 0;

  constructor(canvas: HTMLCanvasElement, intensity: number) {
    this.canvas = canvas;
    // 不要传 willReadFrequently：那是为反复读像素准备的，会把画布退回软件光栅
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("雨效需要 2d 画布上下文");
    this.ctx = ctx;
    this.intensity = intensity;
  }

  setIntensity(value: number) {
    const next = Math.max(0, Math.min(2, value));
    if (next === this.intensity) return;
    this.intensity = next;
    this.build(true);
  }

  resize() {
    const host = this.canvas.parentElement;
    const cssW = Math.max(1, Math.round(host ? host.clientWidth : window.innerWidth));
    const cssH = Math.max(1, Math.round(host ? host.clientHeight : window.innerHeight));
    if (cssW < 2 || cssH < 2) return;

    // 雨是柔和的，不必按屏幕原生分辨率渲染；用像素预算卡住后备缓冲的规模，
    // 免得 4K 高倍屏上一块全屏透明画布光清屏就吃掉整帧
    const dpr = window.devicePixelRatio || 1;
    const budget = Math.sqrt(MAX_BACKING_PIXELS / (cssW * cssH));
    const scale = Math.max(MIN_SCALE, Math.min(dpr, MAX_SCALE, budget));
    if (scale === this.scale && cssW === this.cssW && cssH === this.cssH) return;

    this.cssW = cssW;
    this.cssH = cssH;
    this.scale = scale;
    this.canvas.width = Math.round(cssW * scale);
    this.canvas.height = Math.round(cssH * scale);
    // 改动画布尺寸会重置上下文状态，变换与线帽都要重设
    this.ctx.setTransform(scale, 0, 0, scale, 0, 0);
    this.ctx.lineCap = "round";
    this.bokehGradient = null;
    this.build(true);

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
  }

  /** 按当前尺寸与强度重建雨场。尺寸或强度变化时调用，运行中调用会让雨重来一次。 */
  private build(initial: boolean) {
    if (this.cssW < 2 || this.cssH < 2) return;

    const area = this.cssW * this.cssH;
    const total = Math.min(
      MAX_DROPS,
      Math.round(((DROPS_PER_MEGAPIXEL * area) / 1e6) * this.intensity),
    );

    this.layers = RAIN_LAYERS.map((option) => {
      const count = Math.max(0, Math.round(total * option.share));
      const layer: LayerField = {
        option,
        x: new Float32Array(count),
        y: new Float32Array(count),
        z: new Float32Array(count),
        vy: new Float32Array(count),
        count,
      };
      for (let i = 0; i < count; i++) this.spawn(layer, i, initial);
      return layer;
    });

    this.splash = {
      x: new Float32Array(MAX_SPLASHES),
      y: new Float32Array(MAX_SPLASHES),
      life: new Float32Array(MAX_SPLASHES),
      maxR: new Float32Array(MAX_SPLASHES),
      count: 0,
    };

    const bokehCount = Math.min(
      BOKEH.maxCount,
      Math.round(((BOKEH.perMegapixel * area) / 1e6) * this.intensity),
    );
    this.bokeh = {
      x: new Float32Array(bokehCount),
      y: new Float32Array(bokehCount),
      r: new Float32Array(bokehCount),
      speed: new Float32Array(bokehCount),
      alpha: new Float32Array(bokehCount),
      count: bokehCount,
    };
    for (let i = 0; i < bokehCount; i++) this.spawnBokeh(i, initial);

    // 强度调到 0 时循环不再绘制，得在这里把上一帧擦掉，否则雨会冻在画布上
    this.ctx.clearRect(0, 0, this.cssW, this.cssH);
  }

  private spawn(layer: LayerField, i: number, initial: boolean) {
    const option = layer.option;
    const z = option.zMin + Math.random() * (option.zMax - option.zMin);
    layer.z[i] = z;
    layer.vy[i] = option.speedAtZ1 * z * (1 + (Math.random() * 2 - 1) * SPEED_JITTER);
    layer.x[i] = -WRAP_MARGIN + Math.random() * (this.cssW + WRAP_MARGIN * 2);
    layer.y[i] = initial ? Math.random() * this.cssH : -Math.random() * SPAWN_BAND;
  }

  private spawnBokeh(i: number, initial: boolean) {
    const radius = BOKEH.minRadius + Math.random() * (BOKEH.maxRadius - BOKEH.minRadius);
    this.bokeh.x[i] = Math.random() * this.cssW;
    this.bokeh.y[i] = initial ? Math.random() * this.cssH : -radius;
    this.bokeh.r[i] = radius;
    this.bokeh.speed[i] = BOKEH.minSpeed + Math.random() * (BOKEH.maxSpeed - BOKEH.minSpeed);
    this.bokeh.alpha[i] = BOKEH.minAlpha + Math.random() * (BOKEH.maxAlpha - BOKEH.minAlpha);
  }

  private spawnSplash(x: number, y: number, z: number) {
    const splash = this.splash;
    if (splash.count >= MAX_SPLASHES) return;
    const i = splash.count++;
    splash.x[i] = x;
    splash.y[i] = y;
    splash.life[i] = 1;
    splash.maxR[i] = SPLASH.maxRadius * z * (0.7 + Math.random() * 0.6);
  }

  /** 当前风速，深度为 1 时的像素每秒 */
  private windAt(t: number): number {
    let wind = WIND.base;
    for (let i = 0; i < WIND.gust.length; i++) {
      const gust = WIND.gust[i]!;
      wind += gust.amplitude * Math.sin((t / gust.period) * TAU);
    }
    return wind;
  }

  private frame = (timestamp: number) => {
    if (!this.running) return;
    if (this.lastTs === 0) this.lastTs = timestamp;
    const dt = Math.min((timestamp - this.lastTs) / 1000, MAX_STEP);
    this.lastTs = timestamp;
    this.elapsed += dt;

    if (this.hasParticles()) {
      this.update(dt);
      this.draw();
    }
    this.rafId = requestAnimationFrame(this.frame);
  };

  private hasParticles(): boolean {
    return this.bokeh.count > 0 || this.splash.count > 0 || this.layers.some((l) => l.count > 0);
  }

  private update(dt: number) {
    const wind = this.windAt(this.elapsed);
    const groundY = this.cssH * SPLASH.groundRatio;
    const span = this.cssW + WRAP_MARGIN * 2;

    for (const layer of this.layers) {
      const { x, y, z, vy, count } = layer;
      for (let i = 0; i < count; i++) {
        y[i]! += vy[i]! * dt;
        x[i]! += wind * z[i]! * dt;

        // 雨滴头部触地即回收，顺带溅一圈涟漪
        if (y[i]! >= groundY) {
          if (Math.random() < SPLASH.chance) this.spawnSplash(x[i]!, groundY, z[i]!);
          this.spawn(layer, i, false);
          continue;
        }
        if (x[i]! > this.cssW + WRAP_MARGIN) x[i]! -= span;
        else if (x[i]! < -WRAP_MARGIN) x[i]! += span;
      }
    }

    this.updateSplashes(dt);

    const bokeh = this.bokeh;
    for (let i = 0; i < bokeh.count; i++) {
      bokeh.y[i]! += bokeh.speed[i]! * dt;
      bokeh.x[i]! += wind * 0.1 * dt;
      if (bokeh.y[i]! - bokeh.r[i]! > this.cssH) {
        this.spawnBokeh(i, false);
      } else if (bokeh.x[i]! > this.cssW + bokeh.r[i]!) {
        bokeh.x[i]! -= this.cssW + bokeh.r[i]! * 2;
      }
    }
  }

  private updateSplashes(dt: number) {
    const splash = this.splash;
    const step = dt / SPLASH.life;
    for (let i = splash.count - 1; i >= 0; i--) {
      splash.life[i]! -= step;
      if (splash.life[i]! > 0) continue;
      // 尾元素填洞，涟漪的先后顺序没有意义
      const last = --splash.count;
      splash.x[i] = splash.x[last]!;
      splash.y[i] = splash.y[last]!;
      splash.life[i] = splash.life[last]!;
      splash.maxR[i] = splash.maxR[last]!;
    }
  }

  private draw() {
    const ctx = this.ctx;
    ctx.clearRect(0, 0, this.cssW, this.cssH);

    const wind = this.windAt(this.elapsed);
    ctx.strokeStyle = `rgb(${STREAK_COLOR})`;

    for (const layer of this.layers) {
      const { option, x, y, z, vy, count } = layer;
      if (count === 0) continue;

      // 整条拖尾：一次 beginPath 收下全层雨丝，一次 stroke 出图
      ctx.globalAlpha = option.alpha;
      ctx.lineWidth = option.width;
      ctx.beginPath();
      for (let i = 0; i < count; i++) {
        ctx.moveTo(x[i]!, y[i]!);
        ctx.lineTo(x[i]! - wind * z[i]! * SHUTTER_SECONDS, y[i]! - vy[i]! * SHUTTER_SECONDS);
      }
      ctx.stroke();

      // 头部高亮段：同一条路径里只取最前面一小截，压在上一条上形成渐亮的雨滴头
      const head = option.headRatio;
      ctx.globalAlpha = Math.min(1, option.alpha * option.headBoost);
      ctx.lineWidth = option.width * 1.5;
      ctx.beginPath();
      for (let i = 0; i < count; i++) {
        ctx.moveTo(x[i]!, y[i]!);
        ctx.lineTo(
          x[i]! - wind * z[i]! * SHUTTER_SECONDS * head,
          y[i]! - vy[i]! * SHUTTER_SECONDS * head,
        );
      }
      ctx.stroke();
    }

    this.drawSplashes();
    this.drawBokeh();
    ctx.globalAlpha = 1;
  }

  private drawSplashes() {
    const splash = this.splash;
    if (splash.count === 0) return;

    const ctx = this.ctx;
    ctx.strokeStyle = `rgb(${SPLASH_COLOR})`;
    ctx.lineWidth = 1;
    ctx.lineCap = "butt";

    // 涟漪的透明度要随生命周期衰减，而批绘制要求同批同透明度，
    // 于是按生命周期分三档，三档各一次 stroke
    for (let stage = 0; stage < SPLASH_STAGES; stage++) {
      ctx.globalAlpha = SPLASH.alpha[stage] ?? 0;
      ctx.beginPath();
      for (let i = 0; i < splash.count; i++) {
        const age = 1 - splash.life[i]!;
        const bucket = Math.min(SPLASH_STAGES - 1, Math.floor(age * SPLASH_STAGES));
        if (bucket !== stage) continue;
        const ease = 1 - (1 - age) * (1 - age);
        const r = Math.max(0.5, splash.maxR[i]! * ease);
        const rx = splash.x[i]!;
        const ry = splash.y[i]!;
        // 只画上半圈，读起来就是雨点砸在地面而不是浮在屏幕上的圆
        ctx.moveTo(rx - r, ry);
        ctx.ellipse(rx, ry, r, r * SPLASH.flatten, 0, Math.PI, TAU);
      }
      ctx.stroke();
    }
    ctx.lineCap = "round";
  }

  private drawBokeh() {
    const bokeh = this.bokeh;
    if (bokeh.count === 0) return;
    if (!this.bokehGradient) this.ensureBokehGradient();
    const gradient = this.bokehGradient;
    if (!gradient) return;

    const ctx = this.ctx;
    ctx.fillStyle = gradient;
    for (let i = 0; i < bokeh.count; i++) {
      const r = bokeh.r[i]!;
      ctx.globalAlpha = bokeh.alpha[i]!;
      // 渐变定义在单位空间里，靠这次变换复用同一个渐变对象画任意大小的光斑
      ctx.save();
      ctx.translate(bokeh.x[i]!, bokeh.y[i]!);
      ctx.scale(r, r * 0.82);
      ctx.beginPath();
      ctx.arc(0, 0, 1, 0, TAU);
      ctx.fill();
      ctx.restore();
    }
  }

  private ensureBokehGradient() {
    const gradient = this.ctx.createRadialGradient(0, 0, 0, 0, 0, 1);
    gradient.addColorStop(0, `rgba(${BOKEH.color}, 0.5)`);
    gradient.addColorStop(0.45, `rgba(${BOKEH.color}, 0.16)`);
    gradient.addColorStop(1, `rgba(${BOKEH.color}, 0)`);
    this.bokehGradient = gradient;
  }
}

/**
 * 把雨的渲染场挂到画布上：负责尺寸观察、生命周期与可见性。
 *
 * 画布尺寸取自父元素，所以主界面铺满整屏、塞进小容器里也成立。
 */
export function useRain(
  canvasRef: Ref<HTMLCanvasElement | null>,
  props: { enabled?: boolean; intensity?: number },
) {
  let field: RainField | null = null;
  let observer: ResizeObserver | null = null;

  const resize = () => field?.resize();

  const onVisibilityChange = () => {
    if (document.hidden) field?.stop();
    else if (props.enabled ?? true) field?.start();
  };

  onMounted(() => {
    const canvas = canvasRef.value;
    if (!canvas) return;

    field = new RainField(canvas, props.intensity ?? 1);

    // 观察父元素而不是窗口：布局变化（对话框展开、窗口分屏）同样会改变可用区域
    const host = canvas.parentElement;
    if (host && typeof ResizeObserver !== "undefined") {
      observer = new ResizeObserver(resize);
      observer.observe(host);
    } else {
      window.addEventListener("resize", resize);
    }
    document.addEventListener("visibilitychange", onVisibilityChange);

    field.resize();
    if (props.enabled ?? true) field.start();
  });

  watch(
    () => props.intensity,
    (value) => field?.setIntensity(value ?? 1),
  );

  watch(
    () => props.enabled,
    (enabled) => {
      if (enabled && !document.hidden) field?.start();
      else field?.stop();
    },
  );

  onBeforeUnmount(() => {
    observer?.disconnect();
    observer = null;
    window.removeEventListener("resize", resize);
    document.removeEventListener("visibilitychange", onVisibilityChange);
    field?.destroy();
    field = null;
  });
}

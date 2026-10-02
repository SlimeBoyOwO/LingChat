import type { Ref } from "vue";
import { useParticleField, type ParticleProps } from "./useParticleField";
import {
  MAX_BACKING_PIXELS,
  MAX_SCALE,
  MIN_SCALE,
  RAIN_PROFILE,
  SHUTTER_SECONDS,
  SPEED_JITTER,
  SPLASH_COLOR,
  STREAK_COLOR,
  type RainLayerOption,
  type RainProfile,
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
 * 一次闪电的包络形状：三个错开的高斯脉冲叠一条缓慢衰减的尾巴。
 *
 * 真实闪电不是「亮一下」，而是一串间隔几十毫秒、逐次变暗的脉冲。
 * 这个形状就是「像闪电」和「像开灯」的分界，所以它写在引擎里而不是配置里 ——
 * 配置给的是间隔与总时长，形状本身没有可调的意义。
 * at 是本次闪电时长的比例，width 是脉冲宽度（同为比例）。
 */
const FLASH_PULSES = [
  { at: 0, amp: 1, width: 0.035 },
  { at: 0.075, amp: 0.7, width: 0.05 },
  { at: 0.18, amp: 0.4, width: 0.075 },
];

/** 脉冲之后的衰减尾巴幅度，避免第三次脉冲一结束就整个黑下去 */
const FLASH_TAIL = 0.18;

/** 闪电的当前状态。profile 里没配闪电时，这个字段恒为 null */
interface LightningState {
  /** 距离下一次闪电的剩余时间，秒 */
  timer: number;
  /** 本次闪电的剩余时长，秒。小于等于 0 表示当前没有闪电 */
  life: number;
  /** 本次闪电的总时长，秒 */
  total: number;
  /** 本次闪电的强度，决定闪光亮度 */
  power: number;
  /** 折线，每条是一串交替存放的 x/y；null 表示这道雷不画折线 */
  bolt: Float32Array[] | null;
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
  /** 这一场雨的全部观感参数。小雨与雷阵雨的差别全在这里 */
  private readonly profile: RainProfile;

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
  /** 闪光的纵向渐变，屏幕尺寸，跟着后备缓冲一起失效 */
  private lightningGradient: CanvasGradient | null = null;
  /** 闪电状态，profile 里没配闪电时为 null */
  private lightning: LightningState | null = null;

  private wantRun = false;
  private running = false;
  private rafId = 0;
  private lastTs = 0;
  private elapsed = 0;

  constructor(canvas: HTMLCanvasElement, intensity: number, profile: RainProfile = RAIN_PROFILE) {
    this.canvas = canvas;
    // 不要传 willReadFrequently：那是为反复读像素准备的，会把画布退回软件光栅
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("雨效需要 2d 画布上下文");
    this.ctx = ctx;
    this.profile = profile;
    this.intensity = intensity;

    const lightning = profile.lightning;
    if (lightning) {
      this.lightning = {
        // 首雷不等满一个间隔：挂上就该看出这是雷阵雨，而不是一片普通的雨
        timer: lightning.intervalMin * (0.2 + Math.random() * 0.5),
        life: 0,
        total: 0,
        power: 0,
        bolt: null,
      };
    }
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
    // 改动画布尺寸会重置上下文状态，变换、线帽与线接头都要重设
    this.ctx.setTransform(scale, 0, 0, scale, 0, 0);
    this.ctx.lineCap = "round";
    this.ctx.lineJoin = "round";
    this.bokehGradient = null;
    this.lightningGradient = null;
    // 折线顶点是按旧尺寸算的，尺寸变了就作废。调度本身不重置，
    // 否则每调一次强度都会在同一个节拍上补打一次雷
    if (this.lightning) {
      this.lightning.bolt = null;
      this.lightning.life = 0;
    }
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
    this.lightning = null;
  }

  /** 按当前尺寸与强度重建雨场。尺寸或强度变化时调用，运行中调用会让雨重来一次。 */
  private build(initial: boolean) {
    if (this.cssW < 2 || this.cssH < 2) return;

    const area = this.cssW * this.cssH;
    const total = Math.min(
      this.profile.maxDrops,
      Math.round(((this.profile.dropsPerMegapixel * area) / 1e6) * this.intensity),
    );

    this.layers = this.profile.layers.map((option) => {
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

    const bokehCfg = this.profile.bokeh;
    const bokehCount = Math.min(
      bokehCfg.maxCount,
      Math.round(((bokehCfg.perMegapixel * area) / 1e6) * this.intensity),
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
    const cfg = this.profile.bokeh;
    const radius = cfg.minRadius + Math.random() * (cfg.maxRadius - cfg.minRadius);
    this.bokeh.x[i] = Math.random() * this.cssW;
    this.bokeh.y[i] = initial ? Math.random() * this.cssH : -radius;
    this.bokeh.r[i] = radius;
    this.bokeh.speed[i] = cfg.minSpeed + Math.random() * (cfg.maxSpeed - cfg.minSpeed);
    this.bokeh.alpha[i] = cfg.minAlpha + Math.random() * (cfg.maxAlpha - cfg.minAlpha);
  }

  private spawnSplash(x: number, y: number, z: number) {
    const splash = this.splash;
    if (splash.count >= MAX_SPLASHES) return;
    const i = splash.count++;
    splash.x[i] = x;
    splash.y[i] = y;
    splash.life[i] = 1;
    splash.maxR[i] = this.profile.splash.maxRadius * z * (0.7 + Math.random() * 0.6);
  }

  /** 当前风速，深度为 1 时的像素每秒 */
  private windAt(t: number): number {
    const wind = this.profile.wind;
    let value = wind.base;
    for (let i = 0; i < wind.gust.length; i++) {
      const gust = wind.gust[i]!;
      value += gust.amplitude * Math.sin((t / gust.period) * TAU);
    }
    return value;
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
    const groundY = this.cssH * this.profile.splash.groundRatio;
    const splashChance = this.profile.splash.chance;
    const span = this.cssW + WRAP_MARGIN * 2;

    for (const layer of this.layers) {
      const { x, y, z, vy, count } = layer;
      for (let i = 0; i < count; i++) {
        y[i]! += vy[i]! * dt;
        x[i]! += wind * z[i]! * dt;

        // 雨滴头部触地即回收，顺带溅一圈涟漪
        if (y[i]! >= groundY) {
          if (Math.random() < splashChance) this.spawnSplash(x[i]!, groundY, z[i]!);
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

    this.updateLightning(dt);
  }

  /**
   * 闪电的调度。雨本身与它无关，所以放在 update 的最后，也不参与 hasParticles 的判定 ——
   * 雨停了（强度归零）就该整个特效停下来，而不是留一片在空闪的光。
   */
  private updateLightning(dt: number) {
    const cfg = this.profile.lightning;
    const lightning = this.lightning;
    if (!cfg || !lightning) return;

    if (lightning.life > 0) lightning.life -= dt;

    lightning.timer -= dt;
    if (lightning.timer > 0) return;

    // 触发一次。强度只影响闪光亮度；折线按概率出现，远雷不配折线
    lightning.power = 0.45 + Math.random() * 0.55;
    lightning.total = cfg.duration * (0.7 + Math.random() * 0.6);
    lightning.life = lightning.total;
    lightning.timer = cfg.intervalMin + Math.random() * (cfg.intervalMax - cfg.intervalMin);
    if (Math.random() < cfg.boltChance) this.spawnBolt();
    else lightning.bolt = null;
  }

  /**
   * 生成一道折线的顶点。一次闪电算一次，之后每帧只是重画同一组坐标。
   *
   * 主干自上而下逐段横向抖动，另从主干中段岔出一到两条更短更斜的分支。
   * 每条折线是一串交替存放的 x/y，全部塞进数组，绘制时零分配。
   */
  private spawnBolt() {
    const cfg = this.profile.lightning;
    const lightning = this.lightning;
    if (!cfg || !lightning) return;

    const segments = Math.round(
      cfg.boltMinSegments + Math.random() * (cfg.boltMaxSegments - cfg.boltMinSegments),
    );
    const reach =
      this.cssH * (cfg.boltMinReach + Math.random() * (cfg.boltMaxReach - cfg.boltMinReach));
    const step = reach / segments;
    const startX = this.cssW * (0.12 + Math.random() * 0.76);

    const jitter = step * cfg.boltJitter;
    const trunk = new Float32Array((segments + 1) * 2);
    let x = startX;
    for (let i = 0; i <= segments; i++) {
      trunk[i * 2] = x;
      trunk[i * 2 + 1] = i * step;
      // 大多数顶点只小幅偏移、偶尔来一次大跳，才是雷的样子。
      // 每段等幅随机走出来的是一条均匀的裂纹，不是闪电
      x += (Math.random() * 2 - 1) * jitter * (Math.random() < 0.25 ? 2.2 : 0.55);
    }

    const lines: Float32Array[] = [trunk];
    const branches = 1 + Math.floor(Math.random() * 2);
    for (let b = 0; b < branches; b++) {
      // 分支从主干中段岔出，朝起始点外侧走，看起来才像被主干甩出去的
      const from = Math.floor(segments * (0.35 + Math.random() * 0.3));
      const bsegs = 2 + Math.floor(Math.random() * 3);
      const originX = trunk[from * 2]!;
      const originY = trunk[from * 2 + 1]!;
      const dir = originX >= startX ? 1 : -1;
      const bstep = step * (1 + Math.random());
      // 横向张开按分支自身的长度算，才会有明确的斜角。
      // 按主干每段的长度算的话，分支几乎与主干平行，看着像分叉的河而不是雷
      const driftX = dir * bstep * bsegs * cfg.boltDrift * (0.5 + Math.random() * 0.5);

      const branch = new Float32Array((bsegs + 1) * 2);
      for (let i = 0; i <= bsegs; i++) {
        const t = i / bsegs;
        branch[i * 2] = originX + driftX * t + (Math.random() * 2 - 1) * jitter * 0.6;
        branch[i * 2 + 1] = originY + bstep * i;
      }
      lines.push(branch);
    }

    lightning.bolt = lines;
  }

  /** 当前这一瞬间的闪光强度，0 到 1。把脉冲包络与衰减尾巴叠起来就是它的形状。 */
  private flashAt(): number {
    const lightning = this.lightning;
    if (!lightning || lightning.life <= 0) return 0;

    const u = 1 - lightning.life / lightning.total;
    let value = 0;
    for (let i = 0; i < FLASH_PULSES.length; i++) {
      const pulse = FLASH_PULSES[i]!;
      const d = (u - pulse.at) / pulse.width;
      value += pulse.amp * Math.exp(-d * d);
    }
    value += FLASH_TAIL * Math.exp(-u * 4.5);
    return Math.min(1, value) * lightning.power;
  }

  private updateSplashes(dt: number) {
    const splash = this.splash;
    const step = dt / this.profile.splash.life;
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
    // 闪电画在最上层：它是天上来的光，该盖住雨丝与镜头上的光斑
    this.drawLightning();
    ctx.globalAlpha = 1;
  }

  private drawLightning() {
    const cfg = this.profile.lightning;
    const lightning = this.lightning;
    if (!cfg || !lightning) return;

    const flash = this.flashAt();
    if (flash <= 0.002) return;

    const ctx = this.ctx;

    // 整屏压一层冷白的光。用纵向渐变而不是纯色块：云底最亮、往下淡出，
    // 像光从云里透下来，而不是有人开了灯
    if (!this.lightningGradient) {
      const g = ctx.createLinearGradient(0, 0, 0, this.cssH);
      g.addColorStop(0, `rgba(${cfg.color}, 1)`);
      g.addColorStop(0.45, `rgba(${cfg.color}, 0.62)`);
      g.addColorStop(1, `rgba(${cfg.color}, 0.28)`);
      this.lightningGradient = g;
    }
    ctx.fillStyle = this.lightningGradient;
    ctx.globalAlpha = flash * cfg.maxAlpha;
    ctx.fillRect(0, 0, this.cssW, this.cssH);

    const lines = lightning.bolt;
    if (lines) {
      ctx.strokeStyle = `rgb(${cfg.color})`;

      // 一条折线画两遍：先宽而淡的辉光，再窄而亮的芯。
      // 两遍各一次 stroke，分支都收在同一条路径里
      for (let pass = 0; pass < 2; pass++) {
        ctx.globalAlpha = pass === 0 ? flash * 0.4 : flash;
        ctx.lineWidth = pass === 0 ? 6 : 1.6;
        ctx.beginPath();
        for (let n = 0; n < lines.length; n++) {
          const line = lines[n]!;
          ctx.moveTo(line[0]!, line[1]!);
          for (let i = 2; i < line.length; i += 2) {
            ctx.lineTo(line[i]!, line[i + 1]!);
          }
        }
        ctx.stroke();
      }
    }
  }

  private drawSplashes() {
    const splash = this.splash;
    if (splash.count === 0) return;

    const cfg = this.profile.splash;
    const ctx = this.ctx;
    ctx.strokeStyle = `rgb(${SPLASH_COLOR})`;
    ctx.lineWidth = 1;
    ctx.lineCap = "butt";

    // 涟漪的透明度要随生命周期衰减，而批绘制要求同批同透明度，
    // 于是按生命周期分三档，三档各一次 stroke
    for (let stage = 0; stage < SPLASH_STAGES; stage++) {
      ctx.globalAlpha = cfg.alpha[stage] ?? 0;
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
        ctx.ellipse(rx, ry, r, r * cfg.flatten, 0, Math.PI, TAU);
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
    const color = this.profile.bokeh.color;
    const gradient = this.ctx.createRadialGradient(0, 0, 0, 0, 0, 1);
    gradient.addColorStop(0, `rgba(${color}, 0.5)`);
    gradient.addColorStop(0.45, `rgba(${color}, 0.16)`);
    gradient.addColorStop(1, `rgba(${color}, 0)`);
    this.bokehGradient = gradient;
  }
}

/**
 * 把雨的渲染场挂到画布上。
 *
 * profile 决定这是哪一种雨：不传是普通的雨，传雷阵雨那套就是雷阵雨。
 * 尺寸观察与生命周期交给 useParticleField。
 */
export function useRain(
  canvasRef: Ref<HTMLCanvasElement | null>,
  props: ParticleProps,
  profile: RainProfile = RAIN_PROFILE,
) {
  useParticleField(
    canvasRef,
    props,
    (canvas, intensity) => new RainField(canvas, intensity, profile),
  );
}

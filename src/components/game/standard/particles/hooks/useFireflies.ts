import type { Ref } from "vue";
import { useParticleField, type ParticleProps } from "./useParticleField";
import {
  FIREFLY_CONFIG,
  GLOW_PALETTE,
  MAX_BACKING_PIXELS,
  MAX_SCALE,
  MIN_SCALE,
  SPRITE_SIZE,
  type GlowColor,
} from "../config/fireflies";

const TAU = Math.PI * 2;
/** 单帧最大步长，秒。切后台再回来时不让虫子瞬移一大截 */
const MAX_STEP = 0.05;
/** 配色的权重总和，逐只取色时用 */
const PALETTE_TOTAL = GLOW_PALETTE.reduce((sum, c) => sum + c.weight, 0);

/**
 * 预渲染一只萤火虫的光晕。
 *
 * 从芯到边一路衰减的径向渐变。芯子取近白、外圈取黄绿：萤火虫的灯本身是过曝的
 * 白点，只有周围散出来的光是绿的，两个颜色接在一起才像发光体。
 *
 * 衰减用近似指数而不是线性：光在大气里是指数衰减的，线性衰减画出来是一圈
 * 边界分明的圆盘。中间几档的透明度要给足 —— 压太狠的话，远处的虫子缩到几个
 * 像素时整只只剩下一颗白点，光晕先于本体消失，看着就不发光了。
 *
 * 整只虫子烘成一张图，之后逐只只是缩放贴图 —— 比逐只建渐变省一个数量级的
 * 状态切换，也省掉了 save/translate/scale 那一套。光晕本来就是软的，
 * 缩放看不出损失。
 */
function makeGlowSprite(color: GlowColor): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = SPRITE_SIZE;
  canvas.height = SPRITE_SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) return canvas;

  const c = SPRITE_SIZE / 2;
  const grad = ctx.createRadialGradient(c, c, 0, c, c, c);
  grad.addColorStop(0, `rgba(${color.core}, 1)`);
  grad.addColorStop(0.1, `rgba(${color.core}, 1)`);
  grad.addColorStop(0.18, `rgba(${color.core}, 0.62)`);
  grad.addColorStop(0.3, `rgba(${color.halo}, 0.38)`);
  grad.addColorStop(0.45, `rgba(${color.halo}, 0.2)`);
  grad.addColorStop(0.65, `rgba(${color.halo}, 0.085)`);
  grad.addColorStop(0.85, `rgba(${color.halo}, 0.025)`);
  grad.addColorStop(1, `rgba(${color.halo}, 0)`);
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, SPRITE_SIZE, SPRITE_SIZE);

  return canvas;
}

/** 一群萤火虫。用平铺数组而不是对象数组，热循环里不产生任何垃圾。 */
interface FireflySwarm {
  x: Float32Array;
  y: Float32Array;
  vx: Float32Array;
  vy: Float32Array;
  /** 当前落点 */
  tx: Float32Array;
  ty: Float32Array;
  /** 落点的剩余寿命，秒。到期换一处 */
  rest: Float32Array;
  z: Float32Array;
  radius: Float32Array;
  /** 峰值透明度，乘在闪光包络上 */
  peak: Float32Array;
  /** 一个完整明暗周期的时长，秒 */
  period: Float32Array;
  /** 暗下来时的底色亮度，占峰值的比例 */
  base: Float32Array;
  /** 闪光相位，0 到 1 */
  phase: Float32Array;
  /** 悬停摇摆的相位，弧度 */
  idlePhase: Float32Array;
  /** 航向摆动分量的相位，弧度。与摇摆分开，两者才不会绑在一起动 */
  curlPhase: Float32Array;
  /** 用哪一套配色 */
  variant: Uint8Array;
  count: number;
}

/**
 * 萤火虫的渲染场。
 *
 * 每帧三件事：推进游走、算明暗包络、把每只贴上去。全场共用几张贴图，
 * 逐只只是缩放、透明度与色号不同，所以真正的成本在它们盖住的像素上，不在数量上。
 *
 * 没有任何一只是全黑的 —— 暗的时候压在底色上，所以每一帧都要画。
 */
class FireflyField {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly sprites: HTMLCanvasElement[];

  private field: FireflySwarm = {
    x: new Float32Array(0),
    y: new Float32Array(0),
    vx: new Float32Array(0),
    vy: new Float32Array(0),
    tx: new Float32Array(0),
    ty: new Float32Array(0),
    rest: new Float32Array(0),
    z: new Float32Array(0),
    radius: new Float32Array(0),
    peak: new Float32Array(0),
    period: new Float32Array(0),
    base: new Float32Array(0),
    phase: new Float32Array(0),
    idlePhase: new Float32Array(0),
    curlPhase: new Float32Array(0),
    variant: new Uint8Array(0),
    count: 0,
  };

  private intensity: number;
  private cssW = 0;
  private cssH = 0;
  private scale = 0;

  private wantRun = false;
  private running = false;
  private rafId = 0;
  private lastTs = 0;
  private elapsed = 0;

  constructor(canvas: HTMLCanvasElement, intensity: number) {
    this.canvas = canvas;
    // 不要传 willReadFrequently：那是为反复读像素准备的，会把画布退回软件光栅
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("萤火虫需要 2d 画布上下文");
    this.ctx = ctx;
    this.intensity = intensity;
    this.sprites = GLOW_PALETTE.map(makeGlowSprite);
  }

  setIntensity(value: number) {
    const next = Math.max(0, Math.min(2, value));
    if (next === this.intensity) return;
    this.intensity = next;
    this.build();
  }

  resize() {
    const host = this.canvas.parentElement;
    const cssW = Math.max(1, Math.round(host ? host.clientWidth : window.innerWidth));
    const cssH = Math.max(1, Math.round(host ? host.clientHeight : window.innerHeight));
    if (cssW < 2 || cssH < 2) return;

    const dpr = window.devicePixelRatio || 1;
    const budget = Math.sqrt(MAX_BACKING_PIXELS / (cssW * cssH));
    const scale = Math.max(MIN_SCALE, Math.min(dpr, MAX_SCALE, budget));
    if (scale === this.scale && cssW === this.cssW && cssH === this.cssH) return;

    this.cssW = cssW;
    this.cssH = cssH;
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
    this.field.count = 0;
  }

  /** 按当前尺寸与强度重新布一群虫子。尺寸或强度变化时调用，运行中调用会让整群重来 */
  private build() {
    if (this.cssW < 2 || this.cssH < 2) return;

    const cfg = FIREFLY_CONFIG;
    const area = this.cssW * this.cssH;
    // 数量下限只在按面积算出来还不足的时候兜底，不能盖过强度 ——
    // 直接 Math.max 的话强度归零仍会留下 minCount 只，特效关不掉
    const raw = Math.round(((cfg.perMegapixel * area) / 1e6) * this.intensity);
    const count = raw <= 0 ? 0 : Math.min(cfg.maxCount, Math.max(cfg.minCount, raw));

    this.field = {
      x: new Float32Array(count),
      y: new Float32Array(count),
      vx: new Float32Array(count),
      vy: new Float32Array(count),
      tx: new Float32Array(count),
      ty: new Float32Array(count),
      rest: new Float32Array(count),
      z: new Float32Array(count),
      radius: new Float32Array(count),
      peak: new Float32Array(count),
      period: new Float32Array(count),
      base: new Float32Array(count),
      phase: new Float32Array(count),
      idlePhase: new Float32Array(count),
      curlPhase: new Float32Array(count),
      variant: new Uint8Array(count),
      count,
    };
    for (let i = 0; i < count; i++) this.spawn(i);

    // 强度调到 0 时循环不再绘制，得在这里把上一帧擦掉，否则虫子会冻在画布上
    this.ctx.clearRect(0, 0, this.cssW, this.cssH);
  }

  private spawn(i: number) {
    const cfg = FIREFLY_CONFIG;
    const f = this.field;
    const z = cfg.zMin + Math.random() * (cfg.zMax - cfg.zMin);

    f.z[i] = z;
    f.x[i] = Math.random() * this.cssW;
    f.y[i] = Math.random() * this.cssH;
    f.vx[i] = 0;
    f.vy[i] = 0;
    f.radius[i] =
      cfg.radiusAtZ1 * z * (1 - cfg.radiusJitter + Math.random() * cfg.radiusJitter * 2);
    f.peak[i] = cfg.minPeak + Math.random() * (cfg.maxPeak - cfg.minPeak);
    f.period[i] =
      (cfg.minPeriod + Math.random() * (cfg.maxPeriod - cfg.minPeriod)) *
      (1 + (Math.random() * 2 - 1) * cfg.periodJitter);
    f.base[i] = cfg.minBase + Math.random() * (cfg.maxBase - cfg.minBase);
    f.idlePhase[i] = Math.random() * TAU;
    f.curlPhase[i] = Math.random() * TAU;
    f.variant[i] = this.pickVariant();

    // 初相位纯粹随机。
    //
    // 试过让相位跟着位置走一条斜向梯度，指望同屏的虫子合拍成一片波。
    // 实测两个毛病：一是波只在头三十秒看得出来（周期各有抖动，很快就各走各的，
    // 相干比从 1.27 掉回 1.03）；二是两个均匀分布相加是三角分布，峰值刚好
    // 落在脉冲最暗的位置，于是开场那十几秒只有一成虫子在亮，比随机还难看。
    // 想真做同步得写脉冲耦合振荡器，那是另一个量级的改动，不值得顺手塞进来
    f.phase[i] = Math.random();

    // 首帧就停在半路，不然一挂载整群虫子会同时从静止开始加速
    f.rest[i] = Math.random() * cfg.maxRest;
    this.retarget(i);
  }

  /**
   * 按权重取一套配色。线性扫一遍就够，配色只有六种。
   * 蓝紫的权重压得很低，指望的是几十只里偶尔跳出一两只，而不是满屏霓虹
   */
  private pickVariant(): number {
    let r = Math.random() * PALETTE_TOTAL;
    for (let i = 0; i < GLOW_PALETTE.length; i++) {
      r -= GLOW_PALETTE[i]!.weight;
      if (r <= 0) return i;
    }
    return GLOW_PALETTE.length - 1;
  }

  /**
   * 选下一个落点：在当前位置附近随机取，再夹进画布。
   *
   * 夹取而不是环绕 —— 萤火虫是观众要盯着看的个体，从一边穿到另一边会非常刺眼。
   * 夹取同时保证了位置本身留在画布内：速度只会朝落点收敛，不会过冲。
   */
  private retarget(i: number) {
    const cfg = FIREFLY_CONFIG;
    const f = this.field;
    const reach = cfg.dartRadiusAtZ1 * f.z[i]!;
    const margin = Math.min(f.radius[i]!, this.cssW * 0.5, this.cssH * 0.5);

    const nx = f.x[i]! + (Math.random() * 2 - 1) * reach;
    const ny = f.y[i]! + (Math.random() * 2 - 1) * reach;
    f.tx[i] = Math.max(margin, Math.min(this.cssW - margin, nx));
    f.ty[i] = Math.max(margin, Math.min(this.cssH - margin, ny));
    f.rest[i] = cfg.minRest + Math.random() * (cfg.maxRest - cfg.minRest);
  }

  private frame = (timestamp: number) => {
    if (!this.running) return;
    if (this.lastTs === 0) this.lastTs = timestamp;
    const dt = Math.min((timestamp - this.lastTs) / 1000, MAX_STEP);
    this.lastTs = timestamp;
    this.elapsed += dt;

    if (this.field.count > 0) {
      this.update(dt);
      this.draw();
    }
    this.rafId = requestAnimationFrame(this.frame);
  };

  private update(dt: number) {
    const cfg = FIREFLY_CONFIG;
    const f = this.field;
    const idleFreq = TAU / cfg.idlePeriod;
    const curlFreq = TAU / cfg.curlPeriod;
    const blend = Math.min(1, cfg.agility * dt);

    for (let i = 0; i < f.count; i++) {
      const z = f.z[i]!;

      // 落点到期就换一处。半空中换向正好是萤火虫那种「突然拐一下」
      f.rest[i]! -= dt;
      if (f.rest[i]! <= 0) this.retarget(i);

      const dx = f.tx[i]! - f.x[i]!;
      const dy = f.ty[i]! - f.y[i]!;
      const dist = Math.hypot(dx, dy);
      // 期望速度指向落点、离得越近越慢，于是接近时自然减速停住 ——
      // 「悬停」不需要单独写一段逻辑，它是这条规则的产物
      const speed = Math.min(cfg.maxSpeedAtZ1 * z, dist * cfg.approach);
      // 再叠一个垂直于航向的分量，大小按一条慢正弦来回换向。
      // 路径于是是一串首尾相接的弧，换向处自然接成 S 形 ——
      // 只朝落点直飞的话，画出来是折线随机游走
      const curl = Math.sin(this.elapsed * curlFreq + f.curlPhase[i]!) * cfg.curl;
      const dvx = dist > 0.001 ? (dx / dist - (dy / dist) * curl) * speed : 0;
      const dvy = dist > 0.001 ? (dy / dist + (dx / dist) * curl) * speed : 0;

      // 速度朝期望速度逼近，而不是直接赋值：转向要有一点惯性才不像被牵着走
      f.vx[i]! += (dvx - f.vx[i]!) * blend;
      f.vy[i]! += (dvy - f.vy[i]!) * blend;

      // 悬停时的一点摇摆，两条周期不成整数比的正弦，合成缓慢游移的椭圆。
      // 少了这一笔，停稳的虫子会是一只彻底不动的亮点
      const idle = f.idlePhase[i]!;
      const swayX = Math.sin(this.elapsed * idleFreq + idle) * cfg.idleAmplitude * z;
      const swayY = Math.sin(this.elapsed * idleFreq * 0.61 + idle * 1.7) * cfg.idleAmplitude * z;

      f.x[i]! += (f.vx[i]! + swayX) * dt;
      f.y[i]! += (f.vy[i]! + swayY * 0.7) * dt;
    }
  }

  /**
   * 当前这一瞬间的脉冲强度，0 到 1。峰值定在周期起点，两侧各占半个周期。
   *
   * 两侧的形状指数不同：上升段给大指数，亮起来干脆；衰减段给小指数，退得拖沓。
   * 同一指数的两条余弦拼起来就是呼吸灯 —— 那条曲线上升和下降一样长。
   * 两段在各自端点都归零，所以接起来是连续的，不会在周期边界上跳一下。
   */
  private pulseAt(i: number): number {
    const f = this.field;
    const u = (f.phase[i]! + this.elapsed / f.period[i]!) % 1;
    // 折到 [-0.5, 0.5)：0 是峰值，负数侧是上升、正数侧是衰减
    const half = u < 0.5 ? u : u - 1;
    const q = 0.5 + 0.5 * Math.cos(half * TAU);
    return Math.pow(q, half < 0 ? FIREFLY_CONFIG.risePower : FIREFLY_CONFIG.fallPower);
  }

  private draw() {
    const ctx = this.ctx;
    const f = this.field;
    ctx.clearRect(0, 0, this.cssW, this.cssH);
    if (f.count === 0) return;

    const cfg = FIREFLY_CONFIG;
    // 加色混合：两只虫子靠近时互相照亮，而不是一只盖住另一只。
    // 光斑类效果都该这么叠，普通叠加会在重叠处留下一道清晰的遮挡边界
    ctx.globalCompositeOperation = "lighter";
    for (let i = 0; i < f.count; i++) {
      const pulse = this.pulseAt(i);
      const base = f.base[i]!;
      // 亮起来时连光晕一起涨大。真光源就是这么干的，只改透明度像是有人在拧调光器
      const r = f.radius[i]! * (cfg.dimScale + (1 - cfg.dimScale) * pulse);
      ctx.globalAlpha = (base + (1 - base) * pulse) * f.peak[i]!;
      ctx.drawImage(this.sprites[f.variant[i]!]!, f.x[i]! - r, f.y[i]! - r, r * 2, r * 2);
    }
    ctx.globalCompositeOperation = "source-over";
    ctx.globalAlpha = 1;
  }
}

/**
 * 把萤火虫的渲染场挂到画布上。尺寸观察与生命周期交给 useParticleField。
 */
export function useFireflies(canvasRef: Ref<HTMLCanvasElement | null>, props: ParticleProps) {
  useParticleField(canvasRef, props, (canvas, intensity) => new FireflyField(canvas, intensity));
}

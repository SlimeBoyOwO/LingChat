import type { Ref } from "vue";
import { useParticleField, type ParticleProps } from "./useParticleField";
import {
  HEAD_SPRITE_SIZE,
  MAX_BACKING_PIXELS,
  MAX_SCALE,
  METEOR_CONFIG,
  METEOR_PALETTE,
  MIN_SCALE,
  TRAIL_SPRITE_HEIGHT,
  TRAIL_SPRITE_WIDTH,
  type MeteorColor,
} from "../config/meteors";

/** 单帧最大步长，秒。切后台再回来时不让流星瞬移一大截 */
const MAX_STEP = 0.05;
/** 头部光晕半径相对拖尾线宽的倍数。收得紧一点，头不该比尾巴抢眼 */
const HEAD_SCALE = 3.5;
/**
 * 拖尾切成几段来画。拖尾是沿着弧铺的，段数越多弧越平滑，
 * 代价是每颗流星多几次贴图 —— 量级是每帧几百次小贴图，仍然不是瓶颈
 */
const TAIL_SEGMENTS = 10;
/** 配色的权重总和 */
const PALETTE_TOTAL = METEOR_PALETTE.reduce((sum, c) => sum + c.weight, 0);
/** 同时保留的余迹条数。火流星本来就少见，几条足够 */
const MAX_TRAINS = 6;
/** 余迹的基准透明度。它是残留物，不该亮过流星本身 */
const TRAIN_ALPHA = 0.16;

/**
 * 预渲染一条拖尾。
 *
 * 横向一条从透明渐亮到满的带子，右端最亮 —— 画出去时右端对齐流星头部，
 * 于是尾迹自然从头部往后方淡出。纵向再用一道中间实、两边虚的遮罩压一遍，
 * 免得它是一条硬边直线。
 *
 * 亮度集中在靠头部的一侧：等比的线性渐变会让整条尾巴亮得很平均，
 * 看着像一根棍子而不是划过的轨迹。
 *
 * 同一条尾巴上的色相是流动的：尾端取相邻的那个色相，往头部过渡到本色的光晕与亮芯。
 * 六种配色各自这样绕一圈，同屏几道流星的颜色才是活的，而不是六条单色的线。
 */
function makeTrailSprite(color: MeteorColor): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = TRAIL_SPRITE_WIDTH;
  canvas.height = TRAIL_SPRITE_HEIGHT;
  const ctx = canvas.getContext("2d");
  if (!ctx) return canvas;

  const w = TRAIL_SPRITE_WIDTH;
  const h = TRAIL_SPRITE_HEIGHT;

  const along = ctx.createLinearGradient(0, 0, w, 0);
  along.addColorStop(0, `rgba(${color.tail}, 0)`);
  along.addColorStop(0.28, `rgba(${color.tail}, 0.2)`);
  along.addColorStop(0.6, `rgba(${color.glow}, 0.58)`);
  along.addColorStop(0.85, `rgba(${color.core}, 0.9)`);
  along.addColorStop(1, `rgba(${color.core}, 1)`);
  ctx.fillStyle = along;
  ctx.fillRect(0, 0, w, h);

  // 纵向压软：拖尾是散开的发光气体，不该有上下两条硬边
  ctx.globalCompositeOperation = "destination-in";
  const across = ctx.createLinearGradient(0, 0, 0, h);
  across.addColorStop(0, "rgba(255, 255, 255, 0)");
  across.addColorStop(0.5, "rgba(255, 255, 255, 1)");
  across.addColorStop(1, "rgba(255, 255, 255, 0)");
  ctx.fillStyle = across;
  ctx.fillRect(0, 0, w, h);
  ctx.globalCompositeOperation = "source-over";

  return canvas;
}

/**
 * 预渲染一颗流星的头。
 *
 * 比萤火虫那颗收得紧得多：流星头是个高速烧蚀的点，光晕短促而集中，
 * 铺开成一大团反而变成又一颗萤火虫了。
 */
function makeHeadSprite(color: MeteorColor): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = HEAD_SPRITE_SIZE;
  canvas.height = HEAD_SPRITE_SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) return canvas;

  const c = HEAD_SPRITE_SIZE / 2;
  const grad = ctx.createRadialGradient(c, c, 0, c, c, c);
  grad.addColorStop(0, `rgba(${color.core}, 0.72)`);
  grad.addColorStop(0.14, `rgba(${color.core}, 0.6)`);
  grad.addColorStop(0.3, `rgba(${color.glow}, 0.26)`);
  grad.addColorStop(0.52, `rgba(${color.glow}, 0.08)`);
  grad.addColorStop(0.75, `rgba(${color.glow}, 0.02)`);
  grad.addColorStop(1, `rgba(${color.glow}, 0)`);
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, HEAD_SPRITE_SIZE, HEAD_SPRITE_SIZE);

  return canvas;
}

/** 一群流星。用平铺数组而不是对象数组，热循环里不产生任何垃圾。 */
interface MeteorSwarm {
  x: Float32Array;
  y: Float32Array;
  vx: Float32Array;
  vy: Float32Array;
  /** 出射点，余迹从它拉到头 */
  ox: Float32Array;
  oy: Float32Array;
  /** 拖尾长度，由速度乘拖尾系数推出 */
  len: Float32Array;
  width: Float32Array;
  alpha: Float32Array;
  /** 每秒的转角，弧度。路径是弧而不是直线就靠它 */
  turn: Float32Array;
  variant: Uint8Array;
  fireball: Uint8Array;
  count: number;
}

/**
 * 按容量开一组流星。容量只跟同屏上限有关，与画布尺寸无关，所以开一次就够，
 * 不必像雨和萤火虫那样在尺寸变化时重建。
 */
function makeSwarm(capacity: number): MeteorSwarm {
  return {
    x: new Float32Array(capacity),
    y: new Float32Array(capacity),
    vx: new Float32Array(capacity),
    vy: new Float32Array(capacity),
    ox: new Float32Array(capacity),
    oy: new Float32Array(capacity),
    len: new Float32Array(capacity),
    width: new Float32Array(capacity),
    alpha: new Float32Array(capacity),
    turn: new Float32Array(capacity),
    variant: new Uint8Array(capacity),
    fireball: new Uint8Array(capacity),
    count: 0,
  };
}

/** 火流星走过后留在天上的那道余迹。流星本体被回收了，它还在慢慢暗下去 */
interface Train {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  width: number;
  variant: number;
  /** 剩余时长与总时长，秒 */
  life: number;
  total: number;
}

/**
 * 流星雨的渲染场。
 *
 * 同屏最多十几颗，逐颗绘制完全不是瓶颈。真正决定整片天空像不像一场的，
 * 是几颗之间的关系 —— 方向全场统一、每颗都沿自己的弧慢慢转弯、到达间隔服从
 * 指数分布，三条合起来才是「一场」流星雨。详见 config/meteors.ts。
 */
class MeteorField {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly trails: HTMLCanvasElement[];
  private readonly heads: HTMLCanvasElement[];

  private field: MeteorSwarm = makeSwarm(METEOR_CONFIG.maxActive);
  private readonly trains: Train[] = [];
  private trainCursor = 0;
  /** 圆弧拖尾逐段求折点时的缓存，复用避免逐帧分配 */
  private readonly tailX = new Float64Array(TAIL_SEGMENTS + 1);
  private readonly tailY = new Float64Array(TAIL_SEGMENTS + 1);

  private intensity: number;
  private cssW = 0;
  private cssH = 0;
  private scale = 0;
  /** 距离下一颗流星还有多久，秒 */
  private timer = 0;

  private wantRun = false;
  private running = false;
  private rafId = 0;
  private lastTs = 0;
  /** 上一帧是否真的画了东西，强度归零时靠它决定要不要擦一次 */
  private painted = false;

  constructor(canvas: HTMLCanvasElement, intensity: number) {
    this.canvas = canvas;
    // 不要传 willReadFrequently：那是为反复读像素准备的，会把画布退回软件光栅
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("流星雨需要 2d 画布上下文");
    this.ctx = ctx;
    this.intensity = intensity;
    this.trails = METEOR_PALETTE.map(makeTrailSprite);
    this.heads = METEOR_PALETTE.map(makeHeadSprite);
  }

  setIntensity(value: number) {
    this.intensity = Math.max(0, Math.min(2, value));
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
    // 改动画布尺寸会重置上下文状态，变换、线帽都要重设
    this.ctx.setTransform(scale, 0, 0, scale, 0, 0);
    this.ctx.lineCap = "round";

    // 飞在半路的流星是按旧尺寸的射线飞的，尺寸变了就作废重来
    this.field.count = 0;
    this.trains.length = 0;
    this.timer = 0;
    this.ctx.clearRect(0, 0, cssW, cssH);

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
    this.trains.length = 0;
  }

  /**
   * 两次到达之间的间隔，秒。
   *
   * 陨石撞上大气是泊松过程，间隔服从指数分布。取固定间隔的话流星会踩着
   * 同一个拍子来，长短也均匀，一眼就看出是定时器而不是天象。
   */
  private nextInterval(rate: number): number {
    return -Math.log(1 - Math.min(Math.random(), 0.999)) / rate;
  }

  /**
   * 射线与矩形求交，返回射入与射出的参数。不相交时返回的 near 会大于 far。
   * 用矩形而不是拿一条边去截：方向一旦变了，迎风的边就跟着换，写死某一条边会漏。
   */
  private intersectRect(
    ox: number,
    oy: number,
    cos: number,
    sin: number,
    minX: number,
    minY: number,
    maxX: number,
    maxY: number,
  ): [number, number] {
    let near = -Infinity;
    let far = Infinity;
    if (Math.abs(cos) < 1e-6) {
      if (ox < minX || ox > maxX) return [1, -1];
    } else {
      const a = (minX - ox) / cos;
      const b = (maxX - ox) / cos;
      near = Math.max(near, Math.min(a, b));
      far = Math.min(far, Math.max(a, b));
    }
    if (Math.abs(sin) < 1e-6) {
      if (oy < minY || oy > maxY) return [1, -1];
    } else {
      const a = (minY - oy) / sin;
      const b = (maxY - oy) / sin;
      near = Math.max(near, Math.min(a, b));
      far = Math.min(far, Math.max(a, b));
    }
    return [near, far];
  }

  private spawn() {
    const cfg = METEOR_CONFIG;
    const f = this.field;
    if (f.count >= cfg.maxActive) return;

    // 全场一个方向，只带一点点各自的抖动。各飞各的角度读不出是同一场流星雨，
    // 只像几道随机的划痕
    const angle = cfg.direction + (Math.random() * 2 - 1) * cfg.directionJitter;
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);

    const m = cfg.entryMargin * this.cssW;
    const minX = -m;
    const maxX = this.cssW + m;
    const minY = -m;
    const maxY = this.cssH + m;
    // 迎风的两条边各一条：流星从这两条边上进场
    const horizontalY = sin > 0 ? minY : maxY;
    const verticalX = cos > 0 ? minX : maxX;
    const horizontalSpan = maxX - minX;
    const minPath = cfg.minPathRatio * Math.hypot(this.cssW, this.cssH);

    // 入场点落在迎风角附近的话，流星刚冒出来就出画了，看着像闪了一下。
    // 抽到这种位置就重抽，几次都抽不到就认了，免得死循环
    let x0 = 0;
    let y0 = 0;
    for (let attempt = 0; attempt < 8; attempt++) {
      const pick = Math.random() * (horizontalSpan + (maxY - minY));
      if (pick < horizontalSpan) {
        x0 = minX + pick;
        y0 = horizontalY;
      } else {
        x0 = verticalX;
        y0 = minY + (pick - horizontalSpan);
      }
      const [near, far] = this.intersectRect(x0, y0, cos, sin, 0, 0, this.cssW, this.cssH);
      if (far > near && far - near >= minPath) break;
    }

    const i = f.count++;
    f.ox[i] = x0;
    f.oy[i] = y0;
    f.x[i] = x0;
    f.y[i] = y0;

    const speed = cfg.minSpeed + Math.random() * (cfg.maxSpeed - cfg.minSpeed);
    f.vx[i] = cos * speed;
    f.vy[i] = sin * speed;
    // 拖尾长度跟着速度走，快的自然拖得长。系数远大于任何真实的曝光时间 ——
    // 这一件本来就不是仿真，长拖尾是造型
    f.len[i] = speed * cfg.trailFactor;
    f.turn[i] = cfg.turnRate * (1 + (Math.random() * 2 - 1) * cfg.turnJitter);

    const variant = this.pickVariant();
    f.variant[i] = variant;

    const fireball = Math.random() < cfg.fireballChance;
    f.fireball[i] = fireball ? 1 : 0;
    const boost = fireball ? cfg.fireballBoost : 1;

    f.width[i] = (cfg.minWidth + Math.random() * (cfg.maxWidth - cfg.minWidth)) * boost;
    f.alpha[i] = (cfg.minAlpha + Math.random() * (cfg.maxAlpha - cfg.minAlpha)) * boost;
  }

  /** 按权重取一套配色。线性扫一遍就够，配色只有四种 */
  private pickVariant(): number {
    let r = Math.random() * PALETTE_TOTAL;
    for (let i = 0; i < METEOR_PALETTE.length; i++) {
      r -= METEOR_PALETTE[i]!.weight;
      if (r <= 0) return i;
    }
    return METEOR_PALETTE.length - 1;
  }

  private frame = (timestamp: number) => {
    if (!this.running) return;
    if (this.lastTs === 0) this.lastTs = timestamp;
    const dt = Math.min((timestamp - this.lastTs) / 1000, MAX_STEP);
    this.lastTs = timestamp;

    this.update(dt);
    if (this.field.count > 0 || this.trains.length > 0) {
      this.draw();
      this.painted = true;
    } else if (this.painted) {
      this.ctx.clearRect(0, 0, this.cssW, this.cssH);
      this.painted = false;
    }
    this.rafId = requestAnimationFrame(this.frame);
  };

  private update(dt: number) {
    const cfg = METEOR_CONFIG;
    const f = this.field;

    const rate = cfg.rate * this.intensity;
    if (rate <= 0) {
      // 强度归零就不再生成，同时把计时器归位，免得恢复时一口气补出一堆
      this.timer = 0;
    } else {
      this.timer -= dt;
      while (this.timer <= 0 && f.count < cfg.maxActive) {
        this.spawn();
        this.timer += this.nextInterval(rate);
      }
      // 撞上同屏上限时把计时器压在零，腾出位置就能立刻补上，而不是继续欠账
      if (this.timer < 0) this.timer = 0;
    }

    for (let i = f.count - 1; i >= 0; i--) {
      // 先把速度转一个小角度再走，路径就是一条缓慢转弯的弧而不是直线。
      // 旋转不改变速度大小，所以出场时算好的拖尾长度全程有效
      const step = f.turn[i]! * dt;
      const cos = Math.cos(step);
      const sin = Math.sin(step);
      const vx = f.vx[i]!;
      const vy = f.vy[i]!;
      f.vx[i] = vx * cos - vy * sin;
      f.vy[i] = vx * sin + vy * cos;

      f.x[i]! += f.vx[i]! * dt;
      f.y[i]! += f.vy[i]! * dt;

      // 整条拖尾彻底出画才算飞完。余量给固定值的话，近的快流星会在画外空跑很久，
      // 白白占着同屏名额
      const out = f.len[i]! + this.cssW * 0.05;
      const gone =
        f.x[i]! < -out || f.x[i]! > this.cssW + out || f.y[i]! < -out || f.y[i]! > this.cssH + out;
      if (!gone) continue;

      // 火流星走过后留一道余迹。流星本体马上回收，痕迹还要挂着暗一阵
      if (f.fireball[i]) {
        this.addTrain(f.ox[i]!, f.oy[i]!, f.x[i]!, f.y[i]!, f.width[i]!, f.variant[i]!);
      }
      // 尾元素填洞，流星的先后顺序没有意义
      const last = --f.count;
      f.x[i] = f.x[last]!;
      f.y[i] = f.y[last]!;
      f.vx[i] = f.vx[last]!;
      f.vy[i] = f.vy[last]!;
      f.ox[i] = f.ox[last]!;
      f.oy[i] = f.oy[last]!;
      f.len[i] = f.len[last]!;
      f.width[i] = f.width[last]!;
      f.alpha[i] = f.alpha[last]!;
      f.turn[i] = f.turn[last]!;
      f.variant[i] = f.variant[last]!;
      f.fireball[i] = f.fireball[last]!;
    }

    for (let i = this.trains.length - 1; i >= 0; i--) {
      const train = this.trains[i]!;
      train.life -= dt;
      if (train.life <= 0) {
        this.trains[i] = this.trains[this.trains.length - 1]!;
        this.trains.pop();
      }
    }
  }

  private addTrain(x0: number, y0: number, x1: number, y1: number, width: number, variant: number) {
    const cfg = METEOR_CONFIG;
    const total = cfg.minTrainLife + Math.random() * (cfg.maxTrainLife - cfg.minTrainLife);
    const train: Train = { x0, y0, x1, y1, width, variant, life: total, total };
    // 定长环形缓冲：满了就顶掉最老的一条，不动态增长
    if (this.trains.length < MAX_TRAINS) this.trains.push(train);
    else {
      this.trains[this.trainCursor] = train;
      this.trainCursor = (this.trainCursor + 1) % MAX_TRAINS;
    }
  }

  private draw() {
    const ctx = this.ctx;
    ctx.clearRect(0, 0, this.cssW, this.cssH);

    // 加色混合：流星是光，叠上去而不是盖上去
    ctx.globalCompositeOperation = "lighter";

    this.drawTrains();

    const f = this.field;
    for (let i = 0; i < f.count; i++) {
      const len = f.len[i]!;
      const width = f.width[i]!;
      const variant = f.variant[i]!;

      ctx.globalAlpha = Math.min(1, f.alpha[i]!);
      const speed = Math.hypot(f.vx[i]!, f.vy[i]!) || 1;
      this.drawArcTail(
        f.x[i]!,
        f.y[i]!,
        f.vx[i]! / speed,
        f.vy[i]! / speed,
        speed,
        f.turn[i]!,
        len,
        width,
        variant,
      );

      // 头是一颗圆点，不必跟着转
      const r = width * HEAD_SCALE;
      ctx.drawImage(this.heads[variant]!, f.x[i]! - r, f.y[i]! - r, r * 2, r * 2);
    }

    ctx.globalCompositeOperation = "source-over";
    ctx.globalAlpha = 1;
  }

  /**
   * 余迹复用拖尾那张精灵来画，而不是 stroke 一条线。
   *
   * 用线画出来的是一条边缘锐利、通体等亮的细直线，横贯半个屏幕时就是一道划痕。
   * 余迹实际是散开的发光气体，得是软的 —— 那张精灵本身就是沿长度衰减、
   * 两侧虚化的带子，拉长铺上去正好，连渐变都不用另建。
   */
  /**
   * 沿弧画一条拖尾。
   *
   * 拖尾是流星刚走过的那一段路，那条路本身是弯的，所以不能像直线那样
   * 一次贴图了事 —— 会看到一条弦孤零零地挂在弧的外侧。做法是把拖尾按时间
   * 等分成若干段，从头部沿弧往回逐段推出折点，每段再把精灵上对应的一条切片
   * 转到位贴上。切片是首尾相接取的，合起来仍是原来那条连续渐变的带子，
   * 既没有重复叠加也没有断口。
   *
   * 往回推的方向就是引擎正向走法的逆运算：正向是先转后走，所以反向要
   * 先把航向转回去、再沿它退一段。旋转不改变速度大小，所以每段等长。
   */
  private drawArcTail(
    headX: number,
    headY: number,
    dirX: number,
    dirY: number,
    speed: number,
    turn: number,
    length: number,
    width: number,
    variant: number,
  ) {
    const segLen = length / TAIL_SEGMENTS;
    const back = (-turn * segLen) / speed;
    const cb = Math.cos(back);
    const sb = Math.sin(back);

    const xs = this.tailX;
    const ys = this.tailY;
    let px = headX;
    let py = headY;
    let dx = dirX;
    let dy = dirY;
    xs[0] = px;
    ys[0] = py;
    for (let k = 1; k <= TAIL_SEGMENTS; k++) {
      const nx = dx * cb - dy * sb;
      const ny = dx * sb + dy * cb;
      dx = nx;
      dy = ny;
      px -= dx * segLen;
      py -= dy * segLen;
      xs[k] = px;
      ys[k] = py;
    }

    const ctx = this.ctx;
    const sprite = this.trails[variant]!;
    const srcWidth = TRAIL_SPRITE_WIDTH / TAIL_SEGMENTS;
    // 折点是从头部往回存的，绘制要反过来从尾梢开始，精灵的暗端才对得上
    for (let j = 0; j < TAIL_SEGMENTS; j++) {
      const far = TAIL_SEGMENTS - j;
      const near = far - 1;
      const ax = xs[far]!;
      const ay = ys[far]!;
      const bx = xs[near]!;
      const by = ys[near]!;
      ctx.save();
      ctx.translate(ax, ay);
      ctx.rotate(Math.atan2(by - ay, bx - ax));
      ctx.drawImage(
        sprite,
        j * srcWidth,
        0,
        srcWidth,
        TRAIL_SPRITE_HEIGHT,
        0,
        -width * 0.5,
        segLen,
        width,
      );
      ctx.restore();
    }
  }

  private drawTrains() {
    if (this.trains.length === 0) return;
    const ctx = this.ctx;
    for (const train of this.trains) {
      const fade = TRAIN_ALPHA * (train.life / train.total);
      if (fade <= 0.002) continue;

      const dx = train.x1 - train.x0;
      const dy = train.y1 - train.y0;
      const length = Math.hypot(dx, dy);
      if (length < 1) continue;
      const width = train.width * 2.5;

      ctx.save();
      ctx.translate(train.x0, train.y0);
      ctx.rotate(Math.atan2(dy, dx));
      ctx.globalAlpha = fade;
      ctx.drawImage(this.trails[train.variant]!, 0, -width * 0.5, length, width);
      ctx.restore();
    }
  }
}

/**
 * 把流星雨的渲染场挂到画布上。尺寸观察与生命周期交给 useParticleField。
 */
export function useMeteors(canvasRef: Ref<HTMLCanvasElement | null>, props: ParticleProps) {
  useParticleField(canvasRef, props, (canvas, intensity) => new MeteorField(canvas, intensity));
}

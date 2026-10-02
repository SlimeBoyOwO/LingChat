<template>
  <canvas ref="canvasRef" class="starfield-canvas"></canvas>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  DEFAULT_COLORS,
  DRIFT,
  FOCAL_RATIO,
  GLOW_PASSES,
  MARGIN,
  MAX_BACKING_PIXELS,
  MAX_SCALE,
  MAX_STARS,
  MIN_SCALE,
  MIN_STARS,
  REFERENCE_AREA,
  SCREEN_SPEED_PER_UNIT,
  SHUTTER_SECONDS,
  STAR_LAYERS,
  TWINKLE_GAIN,
  TWINKLE_LEVELS,
  TWINKLE_MAX,
  TWINKLE_MIN,
  TWINKLE_PERIOD,
  type StarLayerOption,
} from "./config/starfield";

const TAU = Math.PI * 2;

/** 单帧最大步长，秒。切后台再回来时不让星空瞬移一大截 */
const MAX_STEP = 0.05;

/** 归一化调色板时用的哨兵色，赋值失败就能看出原字符串不合法 */
const SENTINEL = "#010203";

/** 一层星。平铺数组存状态，热循环里不产生任何对象。 */
interface StarLayer {
  option: StarLayerOption;
  /** 世界坐标，镜头平移时它不动 */
  wx: Float32Array;
  wy: Float32Array;
  /** 焦距除以深度。世界偏移乘它就是屏幕偏移，星点速度与它成正比 */
  k: Float32Array;
  /** 闪烁相位，0 到 1，逐颗错开 */
  phase: Float32Array;
  /** 调色板下标 */
  tint: Uint8Array;
  count: number;
}

/** 把连续亮度量化成有限几档，批绘制才可能有同批同透明度 */
const levelOf = (twinkle: number): number => {
  const bright = TWINKLE_MIN + (TWINKLE_MAX - TWINKLE_MIN) * (twinkle * 0.5 + 0.5);
  const level = (bright * TWINKLE_LEVELS) | 0;
  return level >= TWINKLE_LEVELS ? TWINKLE_LEVELS - 1 : level;
};

/**
 * 星空的渲染场。
 *
 * 星点活在三维空间里，屏幕坐标是透视除法的结果：偏移除以深度再乘焦距。
 * 深度倒数在投胎时就烘进 k，热循环里只剩减法与乘法；每个深度层一组的星点
 * 按颜色与闪烁档位归到同一条路径，一次描绘画完。为此每颗星的线宽与透明度
 * 由它所在的层推出，而不是各存一份。
 */
class StarFieldEngine {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;

  /** 参考星数，按画布面积换算后才是实际星数 */
  private baseCount: number;
  private speed: number;
  private palette: string[] = [];

  private cssW = 0;
  private cssH = 0;
  private scale = 0;
  /** 焦距，像素。屏幕偏移等于世界偏移乘它再除以深度 */
  private focal = 1;

  private layers: StarLayer[] = [];

  // 投影结果的暂存区。批绘制要按颜色与档位重排，位置先算好存下来，
  // 每颗星一帧只做一次透视除法
  private sx = new Float32Array(0);
  private sy = new Float32Array(0);
  private stx = new Float32Array(0);
  private sty = new Float32Array(0);
  private slevel = new Uint8Array(0);

  // 镜头
  private camX = 0;
  private camY = 0;
  private vx = 0;
  private vy = 0;
  private dirX = 1;
  private dirY = 0;
  private elapsed = 0;

  /** 出画判定与投胎的边距，跟着画布短边走，见 config 里的 MARGIN */
  private exitMargin = MARGIN.min;
  private spawnMargin = MARGIN.min * 2;

  private wantRun = false;
  private running = false;
  private rafId = 0;
  private lastTs = 0;

  constructor(
    canvas: HTMLCanvasElement,
    options: { starCount: number; scrollSpeed: number; colors: string[] },
  ) {
    this.canvas = canvas;
    // 不要传 willReadFrequently：那是为反复读像素准备的，会把画布退回软件光栅
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("星空特效需要 2d 画布上下文");
    this.ctx = ctx;

    this.baseCount = options.starCount;
    this.speed = options.scrollSpeed;
    this.palette = this.normalizePalette(options.colors);
  }

  setSpeed(value: number) {
    this.speed = Number.isFinite(value) ? value : 0;
  }

  setCount(value: number) {
    const next = Number.isFinite(value) ? Math.max(0, value) : 0;
    if (next === this.baseCount) return;
    this.baseCount = next;
    this.build();
  }

  /** 换调色板不用重投胎，但下标要收进新调色板的范围 */
  setColors(colors: string[]) {
    const palette = this.normalizePalette(colors);
    this.palette = palette;
    const size = Math.max(1, palette.length);
    for (const layer of this.layers) {
      const { tint, count } = layer;
      for (let i = 0; i < count; i++) tint[i] = tint[i] % size;
    }
  }

  resize() {
    const host = this.canvas.parentElement;
    const cssW = Math.max(1, Math.round(host ? host.clientWidth : window.innerWidth));
    const cssH = Math.max(1, Math.round(host ? host.clientHeight : window.innerHeight));
    if (cssW < 2 || cssH < 2) return;

    // 星点是硬边小亮点，不必按屏幕原生分辨率渲染，但也不能低于一比一，
    // 用像素预算卡住后备缓冲的规模，免得整屏透明画布光清屏就吃掉一帧
    const dpr = window.devicePixelRatio || 1;
    const budget = Math.sqrt(MAX_BACKING_PIXELS / (cssW * cssH));
    const scale = Math.max(MIN_SCALE, Math.min(dpr, MAX_SCALE, budget));
    if (scale === this.scale && cssW === this.cssW && cssH === this.cssH) return;

    this.cssW = cssW;
    this.cssH = cssH;
    this.scale = scale;
    this.canvas.width = Math.round(cssW * scale);
    this.canvas.height = Math.round(cssH * scale);
    // 改动画布尺寸会重置上下文状态，变换、线帽与混合模式都要重设
    this.ctx.setTransform(scale, 0, 0, scale, 0, 0);
    this.ctx.lineCap = "round";
    this.ctx.globalCompositeOperation = "lighter";
    this.focal = Math.min(cssW, cssH) * FOCAL_RATIO;
    const short = Math.min(cssW, cssH);
    this.exitMargin = Math.max(MARGIN.min, Math.min(MARGIN.max, short * MARGIN.ratio));
    this.spawnMargin = this.exitMargin * 2;
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
  }

  private normalizePalette(colors: string[]): string[] {
    const list = Array.isArray(colors) ? colors : [];
    const ctx = this.ctx;
    const kept = ctx.fillStyle;
    const out: string[] = [];
    for (const raw of list) {
      if (typeof raw !== "string") continue;
      // 借上下文把任意合法写法归一化，写不进去说明这个颜色认不出来
      ctx.fillStyle = SENTINEL;
      ctx.fillStyle = raw;
      if (ctx.fillStyle !== SENTINEL) out.push(ctx.fillStyle);
    }
    ctx.fillStyle = kept;
    return out.length > 0 ? out : [...DEFAULT_COLORS];
  }

  /** 按当前尺寸重建星场。尺寸或星数变化时调用，运行中调用会让星空重来一次。 */
  private build() {
    if (this.cssW < 2 || this.cssH < 2) return;

    const total = Math.round(
      Math.min(
        MAX_STARS,
        Math.max(MIN_STARS, (this.baseCount * this.cssW * this.cssH) / REFERENCE_AREA),
      ),
    );

    this.layers = STAR_LAYERS.map((option) => {
      const count = Math.max(0, Math.round(total * option.share));
      const layer: StarLayer = {
        option,
        wx: new Float32Array(count),
        wy: new Float32Array(count),
        k: new Float32Array(count),
        phase: new Float32Array(count),
        tint: new Uint8Array(count),
        count,
      };
      for (let i = 0; i < count; i++) this.spawn(layer, i, true);
      return layer;
    });

    let widest = 0;
    for (const layer of this.layers) widest = Math.max(widest, layer.count);
    this.sx = new Float32Array(widest);
    this.sy = new Float32Array(widest);
    this.stx = new Float32Array(widest);
    this.sty = new Float32Array(widest);
    this.slevel = new Uint8Array(widest);

    // 星数掉到 0 时循环不再绘制，得在这里把上一帧擦掉，否则星空会冻在画布上
    this.ctx.clearRect(0, 0, this.cssW, this.cssH);
  }

  private spawn(layer: StarLayer, i: number, initial: boolean) {
    const option = layer.option;
    const z = option.zMin + Math.random() * (option.zMax - option.zMin);
    const k = this.focal / z;
    layer.k[i] = k;
    layer.phase[i] = Math.random();
    layer.tint[i] = this.palette.length > 1 ? (Math.random() * this.palette.length) | 0 : 0;

    // 首次铺满整屏；之后从画外一点点进场，而不是凭空出现在画面里。
    // 镜头朝 dirX, dirY 走，屏幕上的星点就往反面走，所以进场点在镜头前进的那一侧。
    // 先取屏内随机一点，再沿镜头方向推到刚出画，每颗星的画外行程就都只有几十像素，
    // 不会有一大半星点耗在画外飞进来的路上
    if (initial) {
      this.place(layer, i, k, Math.random() * this.cssW, Math.random() * this.cssH);
      return;
    }
    const x0 = Math.random() * this.cssW;
    const y0 = Math.random() * this.cssH;
    const bx =
      this.dirX > 0
        ? (this.cssW + this.spawnMargin - x0) / this.dirX
        : this.dirX < 0
          ? (-this.spawnMargin - x0) / this.dirX
          : Infinity;
    const by =
      this.dirY > 0
        ? (this.cssH + this.spawnMargin - y0) / this.dirY
        : this.dirY < 0
          ? (-this.spawnMargin - y0) / this.dirY
          : Infinity;
    const t = Math.min(bx, by);
    this.place(layer, i, k, x0 + this.dirX * t, y0 + this.dirY * t);
  }

  /** 把星点放到世界坐标上，使得它在屏幕上正好落在 sx, sy */
  private place(layer: StarLayer, i: number, k: number, sx: number, sy: number) {
    layer.wx[i] = this.camX + (sx - this.cssW * 0.5) / k;
    layer.wy[i] = this.camY + (sy - this.cssH * 0.5) / k;
  }

  private frame = (timestamp: number) => {
    if (!this.running) return;
    if (this.lastTs === 0) this.lastTs = timestamp;
    const dt = Math.min((timestamp - this.lastTs) / 1000, MAX_STEP);
    this.lastTs = timestamp;
    this.elapsed += dt;

    // 星不动，动的是镜头。漂移方向沿正弦来回摆，整片天空才像在缓慢转过
    const angle = Math.sin((this.elapsed / DRIFT.periodSeconds) * TAU) * DRIFT.amplitude;
    this.dirX = Math.cos(angle);
    this.dirY = Math.sin(angle);
    // 世界速度由「深度为 1 的星每秒该在屏幕上走多少像素」反推，于是速度不随窗口大小变
    const worldSpeed = (this.speed * SCREEN_SPEED_PER_UNIT) / this.focal;
    this.vx = this.dirX * worldSpeed;
    this.vy = this.dirY * worldSpeed;
    this.camX += this.vx * dt;
    this.camY += this.vy * dt;

    this.draw();
    this.rafId = requestAnimationFrame(this.frame);
  };

  private draw() {
    const ctx = this.ctx;
    ctx.clearRect(0, 0, this.cssW, this.cssH);
    for (const layer of this.layers) {
      if (layer.count === 0) continue;
      this.project(layer);
      this.paint(layer);
    }
    ctx.globalAlpha = 1;
  }

  /**
   * 投影一层星点，顺手回收跑出画面的。
   *
   * 屏幕上的星点朝镜头前进方向的反面走，所以只有「出画的那一侧正是它正在远离的那一侧」
   * 才收回重投。反过来说，刚投胎还在画外的星位置本来就在画外，一刀切会把它们每帧
   * 都重投一次，永远飘不进来。
   */
  private project(layer: StarLayer) {
    const { wx, wy, k, phase, count } = layer;
    const cx = this.cssW * 0.5;
    const cy = this.cssH * 0.5;
    const camX = this.camX;
    const camY = this.camY;
    const dx = this.dirX;
    const dy = this.dirY;
    const right = this.cssW + this.exitMargin;
    const bottom = this.cssH + this.exitMargin;

    for (let i = 0; i < count; i++) {
      // 闪烁：相位逐颗错开，整片星空才不会一起呼吸
      this.slevel[i] = levelOf(Math.sin((this.elapsed / TWINKLE_PERIOD + phase[i]) * TAU));

      const ki = k[i];
      let x = cx + (wx[i] - camX) * ki;
      let y = cy + (wy[i] - camY) * ki;

      const outX = x > right ? 1 : x < -this.exitMargin ? -1 : 0;
      const outY = y > bottom ? 1 : y < -this.exitMargin ? -1 : 0;
      if (outX * dx < 0 || outY * dy < 0) {
        this.spawn(layer, i, false);
        // 投胎后位置在画外，重投一次，免得把上一帧的残影留在暂存区
        x = cx + (wx[i] - camX) * k[i];
        y = cy + (wy[i] - camY) * k[i];
        this.slevel[i] = levelOf(Math.sin((this.elapsed / TWINKLE_PERIOD + phase[i]) * TAU));
      }

      this.sx[i] = x;
      this.sy[i] = y;
      // 拖尾起点：曝光时间内星点划过的距离，速度越快尾巴越长
      this.stx[i] = x + this.vx * k[i] * SHUTTER_SECONDS;
      this.sty[i] = y + this.vy * k[i] * SHUTTER_SECONDS;
    }
  }

  /**
   * 按颜色与闪烁档位分批绘制一层。
   *
   * 同色的星点收进一条路径，一档一批；每批只改一次颜色，同一条路径再描一遍加宽的
   * 辉光。全场星点因此只有几十次描边，而不是每颗星换一次填充色。
   */
  private paint(layer: StarLayer) {
    const ctx = this.ctx;
    const option = layer.option;
    const tints = this.palette.length;
    const count = layer.count;

    for (let c = 0; c < tints; c++) {
      ctx.strokeStyle = this.palette[c];
      for (let lv = 0; lv < TWINKLE_LEVELS; lv++) {
        ctx.beginPath();
        let used = false;
        for (let i = 0; i < count; i++) {
          if (layer.tint[i] !== c || this.slevel[i] !== lv) continue;
          used = true;
          ctx.moveTo(this.sx[i], this.sy[i]);
          ctx.lineTo(this.stx[i], this.sty[i]);
        }
        if (!used) continue;
        const alpha = option.alpha * TWINKLE_GAIN[lv];
        for (const pass of GLOW_PASSES) {
          ctx.globalAlpha = Math.min(1, alpha * pass.alphaScale);
          ctx.lineWidth = option.width * pass.widthScale;
          ctx.stroke();
        }
      }
    }
  }
}

/**
 * 星空。
 *
 * 三层景深叠出视差，星点逐颗错相闪烁，镜头沿缓慢摆动的方向平移，
 * 速度调大时近景星点还会拉出细丝。绘制细节见本文件里的 StarFieldEngine。
 *
 * 画布尺寸取自父元素，所以主界面铺满整屏、塞进桌宠头像框里也成立。
 */
const props = withDefaults(
  defineProps<{
    enabled?: boolean;
    starCount?: number;
    scrollSpeed?: number;
    colors?: string[];
  }>(),
  {
    enabled: true,
    starCount: 200,
    scrollSpeed: 0.2,
    colors: () => [...DEFAULT_COLORS],
  },
);

const emit = defineEmits(["ready"]);

const canvasRef = ref<HTMLCanvasElement | null>(null);

let engine: StarFieldEngine | null = null;
let observer: ResizeObserver | null = null;

const resize = () => engine?.resize();

const onVisibilityChange = () => {
  if (document.hidden) engine?.stop();
  else if (props.enabled) engine?.start();
};

onMounted(() => {
  const canvas = canvasRef.value;
  if (!canvas) return;

  engine = new StarFieldEngine(canvas, {
    starCount: props.starCount,
    scrollSpeed: props.scrollSpeed,
    colors: props.colors,
  });

  // 观察父元素而不是窗口：布局变化（对话框展开、窗口分屏、塞进小容器）同样会改变可用区域
  const host = canvas.parentElement;
  if (host && typeof ResizeObserver !== "undefined") {
    observer = new ResizeObserver(resize);
    observer.observe(host);
  } else {
    window.addEventListener("resize", resize);
  }
  document.addEventListener("visibilitychange", onVisibilityChange);

  engine.resize();
  if (props.enabled && !document.hidden) engine.start();
  emit("ready", engine);
});

watch(
  () => props.starCount,
  (value) => engine?.setCount(value),
);

watch(
  () => props.scrollSpeed,
  (value) => engine?.setSpeed(value),
);

watch(
  () => props.colors,
  (value) => engine?.setColors(value),
);

watch(
  () => props.enabled,
  (enabled) => {
    if (enabled && !document.hidden) engine?.start();
    else engine?.stop();
  },
);

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
  window.removeEventListener("resize", resize);
  document.removeEventListener("visibilitychange", onVisibilityChange);
  engine?.destroy();
  engine = null;
});
</script>

<style scoped>
.starfield-canvas {
  position: absolute;
  top: 0;
  left: 0;
  display: block;
  width: 100%;
  height: 100%;
  z-index: -1;
  pointer-events: none;
}
</style>

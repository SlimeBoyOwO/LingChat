import {
  MAX_BACKING_PIXELS,
  MAX_SCALE,
  MAX_STEP,
  MIN_SCALE,
  SPAWN_BAND,
  SPRITE_CELL,
  SPRITE_PAD,
  SPRITE_RADIUS,
  WRAP_MARGIN,
} from "../config/falling";
import type { FallingEffect, FallingLayerOption, SpriteSpec } from "../types/falling";

const TAU = Math.PI * 2;

/** 一层粒子。状态平铺进 Float32Array，热循环里不产生任何垃圾 */
interface LayerField {
  option: FallingLayerOption;
  /** 该层容量。数组按它一次性分配，之后只改 count，强度变化不再重新分配 */
  capacity: number;
  count: number;
  x: Float32Array;
  y: Float32Array;
  z: Float32Array;
  vy: Float32Array;
  /** 横摆与翻面的相位 */
  phase: Float32Array;
  /** 横摆角速度 */
  swaySpeed: Float32Array;
  spin: Float32Array;
  spinSpeed: Float32Array;
  /** 翻面角速度，0 表示该粒子不翻面 */
  flipSpeed: Float32Array;
  /** 使用第几张精灵 */
  variant: Uint8Array;
}

/**
 * 下落粒子的渲染场。
 *
 * 性能的关键在于「状态平铺 + 形态预渲染」：粒子状态进 Float32Array，逐帧只做加减；
 * 雪花字形与花瓣在挂载时各画一次到离屏画布并预解码成 ImageBitmap，
 * 逐帧只剩 drawImage 加一次 setTransform。为此牺牲的是逐粒独立的外观参数，
 * 尺寸、落速、透明度都由它所在层的深度推导，不各存一份。
 */
export class FallingField {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly effect: FallingEffect;

  private intensity: number;
  private cssW = 0;
  private cssH = 0;
  private scale = 0;

  private layers: LayerField[] = [];
  /** 预渲染好的形态，构建完成前为空 */
  private sprites: CanvasImageSource[] = [];
  private destroyed = false;

  private wantRun = false;
  private running = false;
  private rafId = 0;
  private lastTs = 0;
  private elapsed = 0;

  constructor(canvas: HTMLCanvasElement, effect: FallingEffect, intensity: number) {
    this.canvas = canvas;
    // 不要传 willReadFrequently：那是为反复读像素准备的，会把画布退回软件光栅
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("下落粒子特效需要 2d 画布上下文");
    this.ctx = ctx;
    this.effect = effect;
    this.intensity = clampIntensity(intensity);
    this.buildSprites();
  }

  setIntensity(value: number) {
    const next = clampIntensity(value);
    if (next === this.intensity) return;
    this.intensity = next;
    this.applyCount(false);
    this.clear();
  }

  resize() {
    const host = this.canvas.parentElement;
    const cssW = Math.max(1, Math.round(host ? host.clientWidth : window.innerWidth));
    const cssH = Math.max(1, Math.round(host ? host.clientHeight : window.innerHeight));
    if (cssW < 2 || cssH < 2) return;

    // 粒子是柔和的，不必按屏幕原生分辨率渲染；用像素预算卡住后备缓冲的规模，
    // 免得 4K 高倍屏上一块全屏透明画布光清屏就吃掉整帧
    const dpr = window.devicePixelRatio || 1;
    const budget = Math.sqrt(MAX_BACKING_PIXELS / (cssW * cssH));
    const scale = Math.max(MIN_SCALE, Math.min(dpr, MAX_SCALE, budget));
    if (scale === this.scale && cssW === this.cssW && cssH === this.cssH) return;

    const oldW = this.cssW;
    this.cssW = cssW;
    this.cssH = cssH;
    this.scale = scale;
    this.canvas.width = Math.round(cssW * scale);
    this.canvas.height = Math.round(cssH * scale);
    // 改动画布尺寸会重置上下文状态，变换与平滑质量都要重设
    this.ctx.setTransform(scale, 0, 0, scale, 0, 0);
    // 精灵尺寸会随粒子尺寸大幅缩下采样，交给高质量过滤，代价只有几百次绘制
    this.ctx.imageSmoothingQuality = "high";

    this.ensureLayers();
    // 已经有粒子时按宽度等比挪位置，比整批重建少一次闪烁
    if (oldW > 0) {
      const kx = cssW / oldW;
      for (const layer of this.layers) {
        for (let i = 0; i < layer.count; i++) layer.x[i]! *= kx;
      }
    }
    this.applyCount(oldW === 0);
    this.clear();

    // 挂载时容器还没有尺寸的话，start 会落空，这里补一次
    if (this.wantRun && !this.running) this.start();
  }

  private clear() {
    if (this.cssW < 2 || this.cssH < 2) return;
    this.ctx.setTransform(this.scale, 0, 0, this.scale, 0, 0);
    this.ctx.clearRect(0, 0, this.cssW, this.cssH);
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
    this.destroyed = true;
    // 位图占的是显存，提前释放比等 GC 稳
    for (const sprite of this.sprites) {
      if (typeof ImageBitmap !== "undefined" && sprite instanceof ImageBitmap) sprite.close();
    }
    this.sprites = [];
    this.layers = [];
  }

  /** 形态只在挂载时画一次。5 张字形或 4 张花瓣被全部粒子复用，不是逐粒一张图。 */
  private buildSprites() {
    const cells = paintSprites(this.effect.sprite);
    if (typeof createImageBitmap !== "function") {
      this.sprites = cells;
      return;
    }
    // 预解码成 ImageBitmap 再逐帧复用：以画布作源每帧都要重新上传纹理，位图则常驻显存
    Promise.all(cells.map((cell) => createImageBitmap(cell)))
      .then((bitmaps) => {
        if (this.destroyed) {
          for (const bitmap of bitmaps) bitmap.close();
          return;
        }
        this.sprites = bitmaps;
      })
      .catch(() => {
        this.sprites = cells;
      });
  }

  private ensureLayers() {
    if (this.layers.length > 0) return;
    this.layers = this.effect.physics.layers.map((option) => {
      const capacity = Math.max(1, Math.ceil(this.effect.physics.maxCount * option.share));
      return {
        option,
        capacity,
        count: 0,
        x: new Float32Array(capacity),
        y: new Float32Array(capacity),
        z: new Float32Array(capacity),
        vy: new Float32Array(capacity),
        phase: new Float32Array(capacity),
        swaySpeed: new Float32Array(capacity),
        spin: new Float32Array(capacity),
        spinSpeed: new Float32Array(capacity),
        flipSpeed: new Float32Array(capacity),
        variant: new Uint8Array(capacity),
      };
    });
  }

  /**
   * 按当前尺寸与强度调整每层的粒子数。
   *
   * 扩容的粒子从画布上方进场，缩容直接裁掉尾部的，都不重建整个场，
   * 所以调强度是渐变而不是整片雪重生一次。
   */
  private applyCount(initial: boolean) {
    const physics = this.effect.physics;
    const area = this.cssW * this.cssH;
    const total = Math.min(
      physics.maxCount,
      Math.round(((physics.perMegapixel * area) / 1e6) * this.intensity),
    );

    for (const layer of this.layers) {
      const want = Math.max(0, Math.min(layer.capacity, Math.round(total * layer.option.share)));
      for (let i = layer.count; i < want; i++) this.spawn(layer, i, initial);
      layer.count = want;
    }
  }

  private spawn(layer: LayerField, i: number, initial: boolean) {
    const physics = this.effect.physics;
    const option = layer.option;
    const z = option.zMin + Math.random() * (option.zMax - option.zMin);
    layer.z[i] = z;
    layer.vy[i] = physics.speedAtZ1 * z;
    layer.x[i] = -WRAP_MARGIN + Math.random() * (this.cssW + WRAP_MARGIN * 2);
    layer.y[i] = initial ? Math.random() * this.cssH : -10 - Math.random() * SPAWN_BAND;
    layer.phase[i] = Math.random() * TAU;
    layer.swaySpeed[i] =
      physics.sway.minSpeed + Math.random() * (physics.sway.maxSpeed - physics.sway.minSpeed);
    layer.spin[i] = Math.random() * TAU;
    layer.spinSpeed[i] =
      physics.spin.minSpeed + Math.random() * (physics.spin.maxSpeed - physics.spin.minSpeed);
    const flip = physics.flip;
    layer.flipSpeed[i] = flip ? flip.minSpeed + Math.random() * (flip.maxSpeed - flip.minSpeed) : 0;
    layer.variant[i] = Math.floor(Math.random() * this.spriteCount());
  }

  private spriteCount(): number {
    const sprite = this.effect.sprite;
    return sprite.kind === "glyph" ? sprite.chars.length : sprite.hues.length;
  }

  /** 当前风速，深度为 1 时的像素每秒 */
  private windAt(t: number): number {
    let wind = this.effect.physics.wind.base;
    const gust = this.effect.physics.wind.gust;
    for (let i = 0; i < gust.length; i++) {
      const item = gust[i]!;
      wind += item.amplitude * Math.sin((t / item.period) * TAU);
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
    for (const layer of this.layers) if (layer.count > 0) return true;
    return false;
  }

  private update(dt: number) {
    const wind = this.windAt(this.elapsed);
    const span = this.cssW + WRAP_MARGIN * 2;
    const bottom = this.cssH + WRAP_MARGIN;

    for (const layer of this.layers) {
      const { x, y, z, vy, spin, spinSpeed, count } = layer;
      for (let i = 0; i < count; i++) {
        y[i]! += vy[i]! * dt;
        x[i]! += wind * z[i]! * dt;
        spin[i]! += spinSpeed[i]! * dt;

        // 落到画面下方就回到顶上，粒子数始终不变
        if (y[i]! > bottom) {
          this.spawn(layer, i, false);
          continue;
        }
        if (x[i]! > this.cssW + WRAP_MARGIN) x[i]! -= span;
        else if (x[i]! < -WRAP_MARGIN) x[i]! += span;
      }
    }
  }

  private draw() {
    const ctx = this.ctx;
    const scale = this.scale;
    // 清屏前先把变换退回基准，否则 clearRect 只擦掉一块被缩放过的区域
    ctx.setTransform(scale, 0, 0, scale, 0, 0);
    ctx.clearRect(0, 0, this.cssW, this.cssH);

    const sprites = this.sprites;
    if (sprites.length === 0) return;

    const physics = this.effect.physics;
    const flip = physics.flip;
    const fade = physics.fade;
    const fadeSpan = Math.max(1, this.cssH * (1 - fade.start));
    const fadeTop = this.cssH * fade.start;
    const t = this.elapsed;

    for (const layer of this.layers) {
      const { x, y, z, phase, swaySpeed, spin, flipSpeed, variant, count } = layer;
      if (count === 0) continue;
      const baseAlpha = layer.option.alpha;

      for (let i = 0; i < count; i++) {
        const depth = z[i]!;
        const quad = physics.sizeAtZ1 * depth * SPRITE_PAD;
        const half = quad / 2;
        const rot = spin[i]!;
        const cos = Math.cos(rot);
        const sin = Math.sin(rot);
        // 翻面：横向宽度按正弦振荡，看起来就是花瓣在三维里翻了个身
        const width = flip
          ? flip.minWidth + (1 - flip.minWidth) * Math.abs(Math.cos(phase[i]! + t * flipSpeed[i]!))
          : 1;
        const drift = (y[i]! - fadeTop) / fadeSpan;
        const fadeMul = 1 - Math.min(1, Math.max(0, drift)) * (1 - fade.endMultiplier);

        ctx.globalAlpha = baseAlpha * fadeMul;
        // 一次 setTransform 直接合成平移、旋转与缩放，省掉逐粒的 save/restore
        ctx.setTransform(
          scale * cos * width,
          scale * sin * width,
          -scale * sin,
          scale * cos,
          scale *
            (x[i]! + Math.sin(phase[i]! + t * swaySpeed[i]!) * physics.sway.amplitude * depth),
          scale * y[i]!,
        );
        ctx.drawImage(sprites[variant[i]!]!, -half, -half, quad, quad);
      }
    }

    ctx.globalAlpha = 1;
    ctx.setTransform(scale, 0, 0, scale, 0, 0);
  }
}

function clampIntensity(value: number): number {
  return Math.max(0, Math.min(2, value));
}

/**
 * 把形态画成若干张离屏画布，一张对应一个字形或一个色相。
 *
 * 带光晕的形态在这里只画一次，逐帧不再碰 shadowBlur 与文字排版，
 * 这也正是原先「每个粒子一个 style 标签」想要却又付不起的效果。
 */
function paintSprites(spec: SpriteSpec): HTMLCanvasElement[] {
  const count = spec.kind === "glyph" ? spec.chars.length : spec.hues.length;
  const cells: HTMLCanvasElement[] = [];

  for (let i = 0; i < count; i++) {
    const cell = document.createElement("canvas");
    cell.width = SPRITE_CELL;
    cell.height = SPRITE_CELL;
    const ctx = cell.getContext("2d");
    if (!ctx) continue;

    ctx.translate(SPRITE_CELL / 2, SPRITE_CELL / 2);
    ctx.shadowColor = spec.glow;
    ctx.shadowBlur = spec.glowBlur;

    if (spec.kind === "glyph") {
      ctx.fillStyle = spec.color;
      ctx.font = `${SPRITE_RADIUS * 2}px ${spec.font}`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      // 第一遍带阴影铺出光晕，第二遍关掉阴影压上实心笔画，字形才有清晰的芯
      ctx.fillText(spec.chars[i]!, 0, 0);
      ctx.shadowBlur = 0;
      ctx.fillText(spec.chars[i]!, 0, 0);
    } else {
      const hue = spec.hues[i]!;
      const gradient = ctx.createLinearGradient(
        -SPRITE_RADIUS,
        -SPRITE_RADIUS,
        SPRITE_RADIUS,
        SPRITE_RADIUS,
      );
      gradient.addColorStop(0, `hsl(${hue}, 100%, ${spec.lightFrom}%)`);
      gradient.addColorStop(1, `hsl(${hue}, 100%, ${spec.lightTo}%)`);
      ctx.fillStyle = gradient;
      petalPath(ctx, SPRITE_RADIUS);
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.fill();
    }

    cells.push(cell);
  }

  return cells;
}

/**
 * 花瓣轮廓：窄长的叶片，顶端一个浅浅的缺口。
 * 缺口不能太深、轮廓不能太宽，否则缩到十几像素就读成一颗心。
 */
function petalPath(ctx: CanvasRenderingContext2D, r: number) {
  ctx.beginPath();
  ctx.moveTo(0, -r * 0.84);
  ctx.bezierCurveTo(r * 0.26, -r * 0.99, r * 0.5, -r * 0.94, r * 0.58, -r * 0.5);
  ctx.bezierCurveTo(r * 0.68, r * 0.15, r * 0.4, r * 0.88, 0, r);
  ctx.bezierCurveTo(-r * 0.4, r * 0.88, -r * 0.68, r * 0.15, -r * 0.58, -r * 0.5);
  ctx.bezierCurveTo(-r * 0.5, -r * 0.94, -r * 0.26, -r * 0.99, 0, -r * 0.84);
  ctx.closePath();
}

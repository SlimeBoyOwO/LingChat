/**
 * 抚摸命中区域与手势识别：指针到部位、部位到晃动方向的纯数学。
 *
 * 与 live2d-interaction.ts 分工：那里算视线，这里算抚摸。两者都不碰引擎与 DOM。
 *
 * 不走引擎的 hitTest：它读 model3 的 HitAreas 声明，而出厂模型一个都没声明，
 * 只会稳定返回空数组，所以区域只能按几何自己推。
 */

/** 归一化命中区域，相对 drawable bounds，原点左上、y 向下。 */
export interface TouchRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** drawable bounds 里用得上的部分，个别 PIXI 版本只有 x/y 没有 minX/minY。 */
export interface TouchBounds {
  minX: number;
  minY: number;
  width: number;
  height: number;
}

/** 命中判定顺序，先到先得。键名沿用 settings.yml 里 body_part 的命名。
    耳朵排在头与身体之后，避免显式写出的耳朵区域抢走普通摸头。 */
export const TOUCH_PART_ORDER: readonly string[] = ["head", "body", "legs", "earLeft", "earRight"];

/**
 * 默认部位模板，以焦点锚点为头部地标。
 *
 * 锚点是视线原点即两眼中点，是唯一能跨模型拿到的「头在哪」的先验。缺锚点时
 * 本模块不生成任何默认区域，因为半身构图的 bounds 正中是躯干，宁可不触发也
 * 不要摸到肚子放摸头动作。
 *
 * 数值按半身立绘的常见比例估的，需要对着目标模型目视校准；偏差由绑定里的
 * 显式 region 覆盖修正。耳朵尾巴一类模型特异性部位推不出来，只能显式写。
 */
const TEMPLATES: Record<string, (anchor: { x: number; y: number }) => TouchRegion> = {
  head: (a) => ({ x: a.x - 0.22, y: a.y - 0.14, width: 0.44, height: 0.34 }),
  body: (a) => ({ x: a.x - 0.32, y: a.y + 0.2, width: 0.64, height: 0.34 }),
  legs: (a) => ({ x: a.x - 0.26, y: a.y + 0.54, width: 0.52, height: 0.46 }),
};

function isCompleteRegion(value: Partial<TouchRegion> | null | undefined): value is TouchRegion {
  return (
    !!value &&
    Number.isFinite(value.x) &&
    Number.isFinite(value.y) &&
    Number.isFinite(value.width) &&
    Number.isFinite(value.height)
  );
}

/** 夹到 0..1，夹紧后没有面积就丢弃。写成 !(delta > 0) 而不是 delta <= 0，顺带挡掉 NaN。 */
function clampRegion(region: TouchRegion): TouchRegion | null {
  const minX = Math.max(0, Math.min(1, region.x));
  const minY = Math.max(0, Math.min(1, region.y));
  const maxX = Math.max(0, Math.min(1, region.x + region.width));
  const maxY = Math.max(0, Math.min(1, region.y + region.height));
  if (!(maxX - minX > 0) || !(maxY - minY > 0)) return null;
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}

/**
 * 解析变体实际生效的部位区域表。
 *
 * 只保留既有绑定又能算出区域的部位：有绑定没区域永远摸不出来，有区域没绑定
 * 摸上去什么都不会发生，两种都提前丢掉。显式 region 覆盖模板，两者同坐标系所以
 * 可以只覆盖其中一个部位。
 */
export function resolveTouchRegions(
  anchor: { x: number; y: number } | null,
  bindings: Record<string, { region?: Partial<TouchRegion> | null }> | null | undefined,
): Record<string, TouchRegion> {
  const regions: Record<string, TouchRegion> = {};
  if (!bindings) return regions;
  for (const [part, binding] of Object.entries(bindings)) {
    if (!binding) continue;
    const template = anchor ? TEMPLATES[part] : undefined;
    const source = isCompleteRegion(binding.region)
      ? binding.region
      : template
        ? template(anchor!)
        : null;
    if (!source) continue;
    const region = clampRegion(source);
    if (region) regions[part] = region;
  }
  return regions;
}

/** 归一化区域换算成模型局部坐标矩形。 */
function regionRect(region: TouchRegion, bounds: TouchBounds) {
  return {
    minX: bounds.minX + bounds.width * region.x,
    minY: bounds.minY + bounds.height * region.y,
    maxX: bounds.minX + bounds.width * (region.x + region.width),
    maxY: bounds.minY + bounds.height * (region.y + region.height),
  };
}

/** 先按 order，再按字典序补上 order 没列出的自定义部位，保证判定顺序可复现。 */
function orderedParts(regions: Record<string, TouchRegion>, order: readonly string[]): string[] {
  const known = order.filter((part) => part in regions);
  const extra = Object.keys(regions)
    .filter((part) => !order.includes(part))
    .sort();
  return [...known, ...extra];
}

/** 模型局部坐标点落在哪个部位，都不在则返回 null。 */
export function hitTouchPart(
  local: { x: number; y: number },
  bounds: TouchBounds,
  regions: Record<string, TouchRegion>,
  order: readonly string[] = TOUCH_PART_ORDER,
): string | null {
  for (const part of orderedParts(regions, order)) {
    const rect = regionRect(regions[part]!, bounds);
    if (
      local.x >= rect.minX &&
      local.x <= rect.maxX &&
      local.y >= rect.minY &&
      local.y <= rect.maxY
    ) {
      return part;
    }
  }
  return null;
}

/**
 * 晃动速度用每秒像素折算成每毫秒，手感因此与鼠标回报率无关：高回报率的设备单次
 * 事件位移更小，按单次位移算会跟着变小，按速度算不会。
 *
 * 幅度上限只给到三成出头，是因为引擎会把它乘 30 写进头部偏航，而呼吸控制器写的
 * 是同一批参数、本身就要占掉 ±15。满偏会一次性顶到参数上限，把呼吸分量在写入
 * 夹紧时整个削掉，头就僵了。
 */
export interface StrokeConfig {
  /** 累计路径长度低于它就不算一次抚摸，用来把顺手划过和真去摸分开。 */
  minDistance: number;
  /** 方向平滑系数，0 到 1，越大越跟手、越小越稳。 */
  smoothing: number;
  /** 参考速度，像素每毫秒。达到它时晃动到满幅。 */
  referenceSpeed: number;
  /** 晃动幅度上限，占焦点满偏的比例。呼吸余量的说明见上方。 */
  amplitude: number;
  /** 手停住多久之后开始回正，毫秒。 */
  idleMs: number;
  /** 回正的时间常数，毫秒，越大回得越慢。 */
  returnMs: number;
}

export const STROKE_DEFAULTS: StrokeConfig = {
  minDistance: 40,
  smoothing: 0.4,
  referenceSpeed: 0.6,
  amplitude: 0.32,
  idleMs: 60,
  returnMs: 130,
};

/** 低于这个幅度就当已经回正，直接归零，免得留下永远衰减不掉的小数。 */
const SETTLED_EPSILON = 0.02;

export interface StrokeEnd {
  part: string | null;
  /** 累计移动是否够得上一次真正的抚摸。 */
  stroked: boolean;
}

export interface StrokeTracker {
  readonly active: boolean;
  /** 已回正到可以忽略。 */
  readonly settled: boolean;
  /** 当前晃动的部位，指针离开可摸区域后为 null（此时仍在手势中，只是没摸着模型）。 */
  readonly part: string | null;
  /** 当前晃动幅度，0 到 1。 */
  readonly magnitude: number;
  /** 当前应喂给焦点控制器的方向，范围 -1..1。未抚摸时随 update 自行衰减回原点。 */
  readonly sway: { x: number; y: number };
  /** 按下，落在部位上才接受。 */
  begin(part: string | null, x: number, y: number, now: number): boolean;
  /** 移动，按瞬时速度更新晃动方向。 */
  move(part: string | null, x: number, y: number, now: number): void;
  /** 抬手，给出这次抚摸的部位与是否够格。晃动不在这里清零，交给 update 自然回正。 */
  end(): StrokeEnd;
  /** 放弃这次手势，同样是自然回正而不是硬切回中。 */
  cancel(): void;
  /** 每帧调用，让晃动随时间回正。 */
  update(now: number): void;
}

export function createStrokeTracker(config: Partial<StrokeConfig> = {}): StrokeTracker {
  const cfg = { ...STROKE_DEFAULTS, ...config };
  const sway = { x: 0, y: 0 };
  let active = false;
  let part: string | null = null;
  let travelled = 0;
  let lastX = 0;
  let lastY = 0;
  let lastMoveAt = Number.NEGATIVE_INFINITY;
  let lastUpdateAt = Number.NEGATIVE_INFINITY;
  let velocityX = 0;
  let velocityY = 0;

  const resetMotion = () => {
    travelled = 0;
    velocityX = 0;
    velocityY = 0;
  };

  const settled = () => Math.abs(sway.x) < SETTLED_EPSILON && Math.abs(sway.y) < SETTLED_EPSILON;

  return {
    get active() {
      return active;
    },
    get settled() {
      return settled();
    },
    get magnitude() {
      return Math.hypot(sway.x, sway.y);
    },
    get part() {
      return part;
    },
    sway,
    begin(nextPart, x, y, now) {
      if (nextPart === null) {
        active = false;
        part = null;
        return false;
      }
      active = true;
      part = nextPart;
      resetMotion();
      lastX = x;
      lastY = y;
      lastMoveAt = now;
      lastUpdateAt = now;
      return true;
    },
    move(nextPart, x, y, now) {
      if (!active) return;
      const dx = x - lastX;
      const dy = y - lastY;
      const dt = Math.max(1, now - lastMoveAt);
      lastX = x;
      lastY = y;
      lastMoveAt = now;
      // 换部位或离开模型就从零起算，别把上一个部位的动量带过来
      if (nextPart === null || nextPart !== part) {
        part = nextPart;
        resetMotion();
        sway.x = 0;
        sway.y = 0;
        return;
      }
      const distance = Math.hypot(dx, dy);
      travelled += distance;
      if (distance <= 0) return;
      velocityX += (dx / dt - velocityX) * cfg.smoothing;
      velocityY += (dy / dt - velocityY) * cfg.smoothing;
      const speed = Math.hypot(velocityX, velocityY);
      if (speed < 1e-6) return;
      // 方向取瞬时速度方向，幅度随速度涨到参考速度为止。手停住时速度趋零，
      // 晃动目标自然回到原点
      const gain = Math.min(1, speed / cfg.referenceSpeed) * cfg.amplitude;
      sway.x = (velocityX / speed) * gain;
      // y 取反：屏幕向下是模型 -Y，与 live2d-interaction 里视线方向用的是同一套
      // 约定。不取反的话上下抚摸会让头往反方向仰
      sway.y = (-velocityY / speed) * gain;
    },
    end() {
      const result = { part, stroked: travelled >= cfg.minDistance };
      active = false;
      part = null;
      resetMotion();
      return result;
    },
    cancel() {
      active = false;
      part = null;
      resetMotion();
    },
    update(now) {
      // 夹住 dt：掉帧之后一次衰减过头会让头瞬移回中
      const dt = Math.min(64, Math.max(0, now - lastUpdateAt));
      lastUpdateAt = now;
      if (active && now - lastMoveAt <= cfg.idleMs) return;
      const decay = Math.exp(-dt / cfg.returnMs);
      sway.x *= decay;
      sway.y *= decay;
      if (settled()) {
        sway.x = 0;
        sway.y = 0;
      }
    },
  };
}

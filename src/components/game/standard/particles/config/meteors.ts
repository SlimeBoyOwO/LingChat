/**
 * 流星雨的调参表。
 *
 * 这一件不做仿真，做的是好看。真实的流星是几毫秒一道直线白光，
 * 放进背景里既看不清也不好看。所以这里刻意偏离物理，往「静谧、美好」上调：
 *
 * 一是**慢**。慢到能看清它划过，而不是闪一下没了。速度一慢，拖尾那条
 * 「长度 = 速度 × 系数」的关系也就跟着变成纯粹的造型手段，系数取值远大于
 * 任何真实的曝光时间。
 *
 * 二是**弧**。路径是一条缓慢转弯的弧，不是直线。全场同号同量级的转角，
 * 弧度才协调 —— 各转各的会散成一团乱麻。
 *
 * 三是**色**。粉、紫、蓝、青、薄荷、暖金，六种轮着来；而且同一条拖尾上
 * 颜色是变的：头部一种色相，往尾端过渡到相邻的另一种。整片天空才是活的颜色，
 * 而不是几条同色的白线。
 *
 * 方向仍然全场统一：同一场流星雨是同一群尘埃平行冲进大气，同屏几道划线
 * 各飞各的角度就读不出是「一场」。到达间隔也仍是泊松过程 —— 均匀间隔会让流星
 * 踩着拍子来，一眼就看出是定时器。
 */

/** 拖尾精灵的尺寸，设备像素。画出去时按每颗流星自己的长度与粗细缩放 */
export const TRAIL_SPRITE_WIDTH = 256;
export const TRAIL_SPRITE_HEIGHT = 64;

/** 头部光晕精灵的边长，设备像素 */
export const HEAD_SPRITE_SIZE = 96;

/**
 * 底下的星辉。只画流星的话画面上是几条线加一大片黑，太空；
 * 垫一层慢慢飘的光点，夜空的底子才撑得住，量要少，多了就抢戏
 */
export const SPARK_COUNT = 40;
export const SPARK_SPEED = 0.25;

/** 画布后备缓冲的像素上限 */
export const MAX_BACKING_PIXELS = 2_500_000;

/** 后备缓冲的最大缩放倍率 */
export const MAX_SCALE = 2;

/** 后备缓冲的最小缩放倍率 */
export const MIN_SCALE = 0.75;

/**
 * 一种流星配色。一条拖尾上有三个色：芯子是头部最亮的一点，
 * 外圈是它周围的光晕，尾端再过渡到第三种色相 —— 同一条尾巴上颜色是流动的。
 */
export interface MeteorColor {
  core: string;
  glow: string;
  /** 拖尾最远端（最淡处）的颜色。取相邻色相，整片天空的颜色才连成谱 */
  tail: string;
  /** 相对权重 */
  weight: number;
}

/**
 * 配色。六种色相绕一圈，权重接近相等，同屏几道流星才会是几种不同的颜色。
 * 每一档的尾端色取的是下一档的色相，于是相邻的流星看上去像从同一条色带上取下来的。
 */
export const METEOR_PALETTE: MeteorColor[] = [
  { core: "255, 240, 252", glow: "255, 152, 206", tail: "188, 150, 255", weight: 1 },
  { core: "240, 234, 255", glow: "182, 158, 255", tail: "128, 186, 255", weight: 1 },
  { core: "226, 242, 255", glow: "124, 188, 255", tail: "116, 230, 246", weight: 1 },
  { core: "230, 252, 255", glow: "116, 228, 242", tail: "150, 248, 200", weight: 1 },
  { core: "234, 255, 246", glow: "138, 246, 188", tail: "255, 216, 150", weight: 1 },
  { core: "255, 248, 228", glow: "255, 212, 142", tail: "255, 168, 214", weight: 0.8 },
];

export interface MeteorConfig {
  /** 平均每秒几颗 */
  rate: number;
  /** 同屏上限，防止某次抽到长间隔后一口气补太多 */
  maxActive: number;

  /**
   * 全场的飞行方向，弧度，画布坐标系（y 轴向下）。
   * 约 2.44 是左下方向、与水平约四十度 —— 斜着才像流星
   */
  direction: number;
  /**
   * 每颗流星相对全场方向的最大偏角，弧度。
   * 现在是 0：全场每一条轨迹都是同一条弧的平移，方向严丝合缝地一致。
   * 这个旋钮留着是为了以后想掺一点自然感时能一处调回来
   */
  directionJitter: number;
  /**
   * 每秒的转角，弧度。路径是一条弧而不是直线就靠它。
   * 全场同号同量级，弧度才协调；正号是逐渐放平，比俯冲更符合想要的调子
   */
  turnRate: number;
  /** 每颗流星转角的相对抖动。同样为 0，弧度也必须完全一致 */
  turnJitter: number;
  /** 入场点到画布边缘的距离，占画布宽度的比例。正数表示在画布外，流星从外面飞进来 */
  entryMargin: number;
  /**
   * 一条轨迹在画布内的可见长度下限，占画布对角线的比例。
   * 入场点落在迎风角附近的话，流星刚出现就出画了，看着像闪了一下；
   * 短于这个长度的入场点会被重抽
   */
  minPathRatio: number;

  /** 飞行速度区间，像素每秒。慢到看得清是这一件的关键 */
  minSpeed: number;
  maxSpeed: number;
  /** 拖尾长度 = 速度 × 它。不是快门时间，只是让快的流星自然拖得长一点 */
  trailFactor: number;
  /** 拖尾线宽区间，CSS 像素 */
  minWidth: number;
  maxWidth: number;

  /** 普通流星的亮度区间 */
  minAlpha: number;
  maxAlpha: number;
  /** 火流星的占比。它更亮、更粗，还会留下余迹 */
  fireballChance: number;
  /** 火流星的亮度与线宽倍数 */
  fireballBoost: number;

  /** 余迹的残留时长区间，秒。火流星走过后那条慢慢暗下去的痕迹 */
  minTrainLife: number;
  maxTrainLife: number;
}

export const METEOR_CONFIG: MeteorConfig = {
  rate: 1.6,
  maxActive: 16,

  direction: 2.44,
  directionJitter: 0,
  turnRate: -0.04,
  turnJitter: 0,
  entryMargin: 0.12,
  minPathRatio: 0.4,

  minSpeed: 280,
  maxSpeed: 680,
  trailFactor: 0.8,
  minWidth: 1.7,
  maxWidth: 3.4,

  minAlpha: 0.55,
  maxAlpha: 0.9,
  fireballChance: 0.12,
  fireballBoost: 1.6,

  minTrainLife: 1.6,
  maxTrainLife: 3.4,
};

/**
 * 萤火虫的调参表。
 *
 * 观感由三件事决定，按重要性排：
 *
 * 一是永不熄灭的底色。第一版把亮度做成「亮一阵、然后归零」，结果每只虫子
 * 亮完就整个黑掉，看上去像一颗一闪而过的光点，不像活物。真实萤火虫暗下来
 * 时也还在，只是弱。所以底下压一层恒定的底色，闪光叠在它上面 ——
 * 这一条决定了屏幕上是「一群一直在的虫子」还是「此起彼伏的光斑」。
 *
 * 二是明暗的节奏。周期、底色占比、以及上升与衰减两段的形状。衰减段要比上升段
 * 胖，亮起来干脆、暗下去拖沓，才不是呼吸灯那条对称的曲线。
 *
 * 三是颜色。一屏虫子同一个色号，一眼就看出是同一张贴图复制出来的，
 * 所以配了一组色：琥珀、暖黄、黄绿、青绿，逐只随机取。
 *
 * 走法反而排在最后：朝一个随机落点缓入、停一会儿、再窜向下一处，比纯正弦的
 * 李萨如曲线更像活的；落点夹在画布内，所以不会漂出屏幕。速度用「期望速度 +
 * 转向速率」逼近，而不是直接改坐标 —— 后者每帧都是随机抖动，看起来像故障。
 */

/** 光晕精灵的边长，设备像素。画出去时会缩放到每只虫子自己的半径 */
export const SPRITE_SIZE = 128;

/** 一种光晕配色。芯子接近过曝的白，外圈才是颜色 —— 灯本身是白的，发光的是周围的空气 */
export interface GlowColor {
  core: string;
  halo: string;
  /** 相对权重。自然色是常客，蓝紫只是偶尔跳出来的点缀 */
  weight: number;
}

/**
 * 光晕配色组。逐只按权重取一种，整片虫子才不会像同一张贴图复制出来的。
 *
 * 前四种是萤火虫实际有的色相：偏橙、正黄、黄绿、偏青。蓝与紫排在后面且权重很低，
 * 只当点缀 —— 它们一多就不再是夏夜草丛，而是霓虹灯。蓝是全白的变体，
 * 紫纯属好看，加进来是为了让一屏里偶尔跳出一只有记忆点的。
 */
export const GLOW_PALETTE: GlowColor[] = [
  { core: "255, 246, 220", halo: "255, 186, 74", weight: 1 },
  { core: "255, 252, 218", halo: "246, 226, 104", weight: 1 },
  { core: "250, 255, 216", halo: "186, 250, 112", weight: 1 },
  { core: "236, 255, 228", halo: "132, 240, 154", weight: 0.7 },
  { core: "224, 242, 255", halo: "92, 176, 255", weight: 0.18 },
  { core: "238, 226, 255", halo: "178, 128, 255", weight: 0.14 },
];

/** 画布后备缓冲的像素上限。萤火虫是一颗软光斑，没必要按 4K 原生分辨率渲染 */
export const MAX_BACKING_PIXELS = 2_500_000;

/** 后备缓冲的最大缩放倍率 */
export const MAX_SCALE = 2;

/** 后备缓冲的最小缩放倍率 */
export const MIN_SCALE = 0.75;

export interface FireflyConfig {
  /** 每百万 CSS 像素的萤火虫数 */
  perMegapixel: number;
  /** 数量上限 */
  maxCount: number;
  /** 数量下限。小窗口按面积算出来只剩零星几只时给个地板 */
  minCount: number;

  /** 深度区间。近的更大更亮、动作幅度也更大，视差由此而来 */
  zMin: number;
  zMax: number;
  /** 深度为 1 时的光晕半径，CSS 像素 */
  radiusAtZ1: number;
  /** 半径的随机浮动比例，免得一屏的虫子一样大 */
  radiusJitter: number;
  /** 单只虫子的峰值透明度区间。有的虫子本来就比同伴暗 */
  minPeak: number;
  maxPeak: number;
  /** 最暗时的尺寸占最亮时的比例。只改亮度的话，暗下去的虫子还占着一块光晕 */
  dimScale: number;

  /** 深度为 1 时的最大飞行速度，像素每秒 */
  maxSpeedAtZ1: number;
  /** 期望速度随距离的增长系数，每秒。越大越早开始减速 */
  approach: number;
  /** 速度追赶期望速度的速率，每秒。越大转向越干脆 */
  agility: number;
  /** 每次选落点时相对当前位置的最大偏移，深度为 1 时 */
  dartRadiusAtZ1: number;
  /**
   * 垂直于航向的摆动分量，相对前进速度的比例。
   * 少了这一笔，虫子就是在几个随机点之间连直线 —— 转角虽然被平滑过，
   * 段与段之间却是直的，整条路径读起来是折线随机游走，不是活物。
   * 给上之后每段都带弧度，正负交替时自然接成 S 形
   */
  curl: number;
  /** 上述摆动换向的周期，秒 */
  curlPeriod: number;
  /** 在一个落点上待的时长区间，秒。到期就换一处 */
  minRest: number;
  maxRest: number;
  /** 悬停时的细微摇摆幅度，像素每秒。防止停稳之后变成一只不动的亮点 */
  idleAmplitude: number;
  /** 摇摆的周期，秒 */
  idlePeriod: number;

  /** 一个完整明暗周期的时长区间，秒 */
  minPeriod: number;
  maxPeriod: number;
  /** 周期的随机浮动比例。周期完全一致的话，整片虫子会永远踩着同一个鼓点 */
  periodJitter: number;
  /** 暗下来时的底色亮度，占峰值的比例。这是虫子「一直都在」的那部分 */
  minBase: number;
  maxBase: number;
  /** 上升段的形状指数，越大越陡。上升要干脆 */
  risePower: number;
  /** 衰减段的形状指数，越小越拖沓。衰减要慢 */
  fallPower: number;
}

export const FIREFLY_CONFIG: FireflyConfig = {
  perMegapixel: 48,
  maxCount: 260,
  minCount: 8,

  zMin: 0.45,
  zMax: 1,
  radiusAtZ1: 24,
  radiusJitter: 0.3,
  minPeak: 0.6,
  maxPeak: 1,
  dimScale: 0.72,

  maxSpeedAtZ1: 95,
  approach: 1.3,
  agility: 3.2,
  dartRadiusAtZ1: 65,
  curl: 0.55,
  curlPeriod: 6.3,
  minRest: 0.7,
  maxRest: 2.4,
  idleAmplitude: 6,
  idlePeriod: 3.7,

  minPeriod: 2.2,
  maxPeriod: 4.4,
  periodJitter: 0.1,
  minBase: 0.16,
  maxBase: 0.32,
  risePower: 2.4,
  fallPower: 1.15,
};

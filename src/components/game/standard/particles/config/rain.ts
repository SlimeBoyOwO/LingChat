/**
 * 雨的调参表。观感参数集中在这里，改数值就能调雨，不必动引擎。
 *
 * 三层景深共用一条物理关系：拖尾长度 = 下落速度 × 快门时间。
 * 远景又慢又短又暗、近景又快又长又亮，视差自然成立，
 * 不用给每层单独凑一组「看起来差不多」的数字。
 */

/** 快门时间，秒。拖尾长度由它乘速度推出，相当于曝光时间内雨丝划过的轨迹。 */
export const SHUTTER_SECONDS = 0.055;

/** 雨丝的冷白色 */
export const STREAK_COLOR = "203, 226, 255";

/** 涟漪的冷白色，比雨丝更亮才看得清 */
export const SPLASH_COLOR = "222, 240, 255";

export interface RainLayerOption {
  /** 该层雨滴占总数比例 */
  share: number;
  /** 深度区间，1 表示贴脸，越小越远 */
  zMin: number;
  zMax: number;
  /** 深度为 1 时的下落速度，像素每秒 */
  speedAtZ1: number;
  /** 整条拖尾的线宽，CSS 像素 */
  width: number;
  /** 整条拖尾的透明度 */
  alpha: number;
  /** 高亮头部占拖尾长度的比例 */
  headRatio: number;
  /** 头部透明度相对整条拖尾的增益 */
  headBoost: number;
}

/** 由远及近三层，顺序即绘制顺序，画在前面的在更远处。 */
export const RAIN_LAYERS: RainLayerOption[] = [
  {
    share: 0.45,
    zMin: 0.28,
    zMax: 0.45,
    speedAtZ1: 1050,
    width: 0.8,
    alpha: 0.13,
    headRatio: 0.3,
    headBoost: 1.7,
  },
  {
    share: 0.35,
    zMin: 0.45,
    zMax: 0.72,
    speedAtZ1: 1050,
    width: 1.25,
    alpha: 0.22,
    headRatio: 0.34,
    headBoost: 1.7,
  },
  {
    share: 0.2,
    zMin: 0.72,
    zMax: 1,
    speedAtZ1: 1050,
    width: 1.9,
    alpha: 0.34,
    headRatio: 0.38,
    headBoost: 1.8,
  },
];

/**
 * 每滴雨的速度抖动比例。
 *
 * 速度与横向漂移都按深度系数缩放，比值恒定，于是全场雨丝的倾角会一模一样。
 * 给落速加一点随机，倾角就跟着散开，这一笔决定了雨看起来是不是印刷品。
 */
export const SPEED_JITTER = 0.16;

/** 每百万像素的雨滴数。密度随画布面积缩放，超宽屏不会显得稀。 */
export const DROPS_PER_MEGAPIXEL = 130;

/** 雨滴总数上限 */
export const MAX_DROPS = 1400;

/** 阵风：两条周期互质的正弦叠加，避免出现听得出循环的节奏 */
export const WIND = {
  /** 基础风速，深度为 1 时的像素每秒，决定雨的倾斜方向与角度 */
  base: 135,
  gust: [
    { amplitude: 58, period: 17.3 },
    { amplitude: 27, period: 6.1 },
  ],
};

/** 落地的涟漪 */
export const SPLASH = {
  /** 地面在画布高度的位置，1 是屏幕底边。对话框会压住底边，想看清可以调小 */
  groundRatio: 1,
  /** 雨滴落地生成涟漪的概率，不是每一滴都值得溅一下 */
  chance: 0.22,
  /** 涟漪最大半径，CSS 像素，远景按深度缩小 */
  maxRadius: 13,
  /** 扩散时长，秒 */
  life: 0.5,
  /** 压扁比例，水面上的涟漪是椭圆 */
  flatten: 0.34,
  /** 三个阶段各自的透明度，刚生成最亮 */
  alpha: [0.34, 0.2, 0.1],
};

/** 贴在镜头玻璃上的雨滴。虚焦的一大团，最出氛围又最不耗性能。 */
export const BOKEH = {
  perMegapixel: 4,
  maxCount: 18,
  /** 半径区间，CSS 像素 */
  minRadius: 16,
  maxRadius: 52,
  /** 沿玻璃下滑的速度区间，像素每秒，比真雨慢得多 */
  minSpeed: 26,
  maxSpeed: 70,
  /** 透明度区间。这一层只是氛围，压过头会变成一颗颗发光球 */
  minAlpha: 0.07,
  maxAlpha: 0.16,
  /** 色彩，填进 rgb() */
  color: "198, 224, 255",
};

/** 画布后备缓冲的像素上限。雨本来就是柔和的，没必要按 4K 原生分辨率渲染。 */
export const MAX_BACKING_PIXELS = 3_000_000;

/** 后备缓冲的最大缩放倍率，三倍屏也不会超过它 */
export const MAX_SCALE = 2;

/** 后备缓冲的最小缩放倍率，再低就糊得不像雨了 */
export const MIN_SCALE = 0.75;

/** 风的参数 */
export interface WindConfig {
  /** 基础风速，深度为 1 时的像素每秒 */
  base: number;
  /** 阵风分量，两条周期互质的正弦叠加 */
  gust: { amplitude: number; period: number }[];
}

/** 落地涟漪的参数 */
export interface SplashConfig {
  groundRatio: number;
  chance: number;
  maxRadius: number;
  life: number;
  flatten: number;
  alpha: number[];
}

/** 镜头玻璃光斑的参数 */
export interface BokehConfig {
  perMegapixel: number;
  maxCount: number;
  minRadius: number;
  maxRadius: number;
  minSpeed: number;
  maxSpeed: number;
  minAlpha: number;
  maxAlpha: number;
  color: string;
}

/** 闪电的参数，只有雷阵雨用得上 */
export interface LightningConfig {
  /** 两次闪电之间间隔的区间，秒 */
  intervalMin: number;
  intervalMax: number;
  /** 一次闪电的持续时长，秒，含数次急促脉冲与之后的缓慢衰减 */
  duration: number;
  /** 闪光峰值透明度 */
  maxAlpha: number;
  /** 闪光颜色，填进 rgb() */
  color: string;
  /** 触发分叉闪电的概率。远处的雷只闪不画，不给每一道雷都配折线 */
  boltChance: number;
  /** 折线主干段数区间 */
  boltMinSegments: number;
  boltMaxSegments: number;
  /** 折线垂直延伸占画布高度的比例区间 */
  boltMinReach: number;
  boltMaxReach: number;
  /**
   * 顶点横向抖动的幅度，相对每段的长度而不是像素 ——
   * 绝对像素在小画布上会把折线抖成一团锯齿，比例才随尺寸一起缩放
   */
  boltJitter: number;
  /** 分支横向张开的幅度，同样相对每段的长度。太小贴着主干，太大像另一道雷 */
  boltDrift: number;
}

/**
 * 一套完整的雨参数。引擎按它渲染，换一个 profile 就是另一种雨。
 *
 * 拖尾长度（快门时间）、速度抖动、颜色与后备缓冲预算不在这里 ——
 * 那些是引擎自身的常量，几种雨共用。
 */
export interface RainProfile {
  layers: RainLayerOption[];
  dropsPerMegapixel: number;
  maxDrops: number;
  wind: WindConfig;
  splash: SplashConfig;
  bokeh: BokehConfig;
  /** 有值才会打雷闪电 */
  lightning?: LightningConfig;
}

/** 默认的雨。数值与收成 profile 之前逐字一致。 */
export const RAIN_PROFILE: RainProfile = {
  layers: RAIN_LAYERS,
  dropsPerMegapixel: DROPS_PER_MEGAPIXEL,
  maxDrops: MAX_DROPS,
  wind: WIND,
  splash: SPLASH,
  bokeh: BOKEH,
};

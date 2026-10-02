/**
 * 雷阵雨的调参表。
 *
 * 它跑的是和雨同一台引擎（hooks/useRain.ts），差别只在这里：
 * 雨更密更快、风更大更斜，另加一层闪电。拖尾长度、扩散的涟漪、
 * 镜头玻璃上的光斑这些机制两者共用，所以这里只写数值。
 *
 * 与小雨（config/rain.ts）的分工：小雨是氛围，雷阵雨是天气。
 */

import type { RainLayerOption, RainProfile } from "./rain";

/**
 * 由远及近三层。比小雨的每一层都更宽更亮，近景占比也更大 ——
 * 暴雨的体感来自「近处的雨丝糊住视野」，而不是全场均匀加密。
 * 落速提到 1250，配合更大的风速，雨丝的倾角从约 7 度张开到 10 度以上。
 */
const STORM_LAYERS: RainLayerOption[] = [
  {
    share: 0.42,
    zMin: 0.3,
    zMax: 0.48,
    speedAtZ1: 1250,
    width: 1,
    alpha: 0.16,
    headRatio: 0.3,
    headBoost: 1.7,
  },
  {
    share: 0.34,
    zMin: 0.48,
    zMax: 0.74,
    speedAtZ1: 1250,
    width: 1.5,
    alpha: 0.26,
    headRatio: 0.34,
    headBoost: 1.75,
  },
  {
    share: 0.24,
    zMin: 0.74,
    zMax: 1.05,
    speedAtZ1: 1250,
    width: 2.2,
    alpha: 0.4,
    headRatio: 0.38,
    headBoost: 1.85,
  },
];

export const THUNDERSTORM_PROFILE: RainProfile = {
  layers: STORM_LAYERS,

  /** 密度约为小雨的 1.85 倍。再密下去就会糊住背景画，galgame 里背景也是要看的 */
  dropsPerMegapixel: 240,
  maxDrops: 2600,

  /** 风比小雨大一倍，阵风幅度也更大，雨幕整体被吹得歪向一侧 */
  wind: {
    base: 235,
    gust: [
      { amplitude: 95, period: 13.7 },
      { amplitude: 44, period: 5.3 },
    ],
  },

  splash: {
    groundRatio: 1,
    chance: 0.3,
    maxRadius: 15,
    life: 0.45,
    flatten: 0.34,
    alpha: [0.38, 0.22, 0.11],
  },

  bokeh: {
    perMegapixel: 6,
    maxCount: 26,
    minRadius: 18,
    maxRadius: 60,
    minSpeed: 34,
    maxSpeed: 88,
    minAlpha: 0.08,
    maxAlpha: 0.18,
    color: "198, 224, 255",
  },

  /**
   * 闪电。真实闪电不是「亮一下」，而是一串间隔几十毫秒、逐次变暗的
   * 脉冲，之后拖一条缓慢衰减的尾巴 —— 引擎按这个形状拟合包络，
   * 所以这里给的是间隔与时长，不是某条曲线。
   *
   * 5 到 16 秒的间隔是刻意拉开的：太密会从「天气」变成「闪烁」。
   */
  lightning: {
    intervalMin: 5,
    intervalMax: 16,
    duration: 0.95,
    maxAlpha: 0.42,
    color: "214, 232, 255",
    boltChance: 0.45,
    boltMinSegments: 9,
    boltMaxSegments: 15,
    boltMinReach: 0.5,
    boltMaxReach: 0.95,
    boltJitter: 0.75,
    boltDrift: 1.1,
  },
};

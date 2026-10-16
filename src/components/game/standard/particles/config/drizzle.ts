/**
 * 小雨的调参表。
 *
 * 它跑的是和雨同一台引擎（hooks/useRain.ts），差别只是雨丝更少、更细、更慢。
 * 不是把雨的密度调低那么简单：雨滴小一倍，落速、受风的影响、溅起的涟漪
 * 都要跟着降，否则稀疏的大雨滴看起来只是「雨画少了」，不像小雨。
 */

import type { RainLayerOption, RainProfile } from "./rain";

/**
 * 三层景深与雨一致，但权重重心更靠远层 ——
 * 小雨的观感是一片细密的远雾，不是几根砸在眼前的水柱。
 */
const DRIZZLE_LAYERS: RainLayerOption[] = [
  {
    share: 0.5,
    zMin: 0.28,
    zMax: 0.45,
    speedAtZ1: 780,
    width: 0.65,
    alpha: 0.1,
    headRatio: 0.28,
    headBoost: 1.7,
  },
  {
    share: 0.32,
    zMin: 0.45,
    zMax: 0.72,
    speedAtZ1: 780,
    width: 0.95,
    alpha: 0.16,
    headRatio: 0.32,
    headBoost: 1.7,
  },
  {
    share: 0.18,
    zMin: 0.72,
    zMax: 1,
    speedAtZ1: 780,
    width: 1.4,
    alpha: 0.24,
    headRatio: 0.36,
    headBoost: 1.8,
  },
];

export const DRIZZLE_PROFILE: RainProfile = {
  layers: DRIZZLE_LAYERS,

  /** 密度约为雨的六成，这是它和雨最直观的区别 */
  dropsPerMegapixel: 70,
  maxDrops: 800,

  /** 风也小得多，雨丝几乎是直着落下来的 */
  wind: {
    base: 80,
    gust: [
      { amplitude: 34, period: 17.3 },
      { amplitude: 16, period: 6.1 },
    ],
  },

  splash: {
    groundRatio: 1,
    chance: 0.16,
    maxRadius: 10,
    life: 0.55,
    flatten: 0.34,
    alpha: [0.26, 0.15, 0.08],
  },

  /** 镜头上的水珠也少一些、小一些：小雨在玻璃上挂不住大颗 */
  bokeh: {
    perMegapixel: 2.5,
    maxCount: 12,
    minRadius: 14,
    maxRadius: 44,
    minSpeed: 20,
    maxSpeed: 54,
    minAlpha: 0.06,
    maxAlpha: 0.13,
    color: "198, 224, 255",
  },
};

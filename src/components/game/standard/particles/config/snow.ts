import type { FallingEffect } from "../types/falling";

/**
 * 雪的调参表。观感参数集中在这里，改数值就能调雪，不必动引擎。
 *
 * 三层景深共用一条物理关系：同一深度下尺寸、落速、横摆幅度一起缩放。
 * 远景又小又慢又淡、近景又快又亮，视差自然成立，
 * 不用给每层单独凑一组「看起来差不多」的数字。
 *
 * 落速取 62 像素每秒：中等深度约 35 像素每秒，一屏飘完约 26 秒，
 * 与旧实现的 20 到 35 秒时长区间相当。
 */
export const snow: FallingEffect = {
  physics: {
    layers: [
      { share: 0.5, zMin: 0.3, zMax: 0.5, alpha: 0.5 },
      { share: 0.3, zMin: 0.5, zMax: 0.75, alpha: 0.62 },
      { share: 0.2, zMin: 0.75, zMax: 1, alpha: 0.78 },
    ],
    speedAtZ1: 62,
    sizeAtZ1: 30,
    /** 横向风。雪的密度低，风只把落点吹斜十来度，不做成被风卷着的暴雪 */
    wind: {
      base: 12,
      gust: [
        { amplitude: 9, period: 19.7 },
        { amplitude: 4, period: 7.3 },
      ],
    },
    /** 逐粒正弦横摆，雪「飘」起来主要靠它 */
    sway: { amplitude: 24, minSpeed: 0.35, maxSpeed: 0.85 },
    spin: { minSpeed: -0.55, maxSpeed: 0.55 },
    /** 快出画面时淡出，避免在底边整齐地消失 */
    fade: { start: 0.72, endMultiplier: 0.4 },
    perMegapixel: 60,
    maxCount: 300,
  },
  sprite: {
    kind: "glyph",
    chars: ["❄", "❅", "❆", "•", "·"],
    color: "#ffffff",
    glow: "rgba(226, 240, 255, 0.65)",
    glowBlur: 12,
    font: '"Segoe UI Symbol", "Noto Sans Symbols 2", "Apple Symbols", sans-serif',
  },
};

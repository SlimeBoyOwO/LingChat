import type { FallingEffect } from "../types/falling";

/**
 * 樱花的调参表。与雪共用引擎，只有落速、尺寸、配色与翻面不同。
 *
 * 比雪重一点、快一点，另外多了 flip：花瓣绕自身竖轴翻面，
 * 靠横向宽度的正弦振荡伪造，这一笔是花瓣看起来在「打转」而非「贴纸平移」的关键。
 */
export const sakura: FallingEffect = {
  physics: {
    layers: [
      { share: 0.5, zMin: 0.32, zMax: 0.55, alpha: 0.55 },
      { share: 0.3, zMin: 0.55, zMax: 0.78, alpha: 0.68 },
      { share: 0.2, zMin: 0.78, zMax: 1, alpha: 0.82 },
    ],
    /** 中等深度约 55 像素每秒，一屏飘完约 17 秒，与旧实现的 15 到 25 秒相当 */
    speedAtZ1: 85,
    sizeAtZ1: 24,
    wind: {
      base: 18,
      gust: [
        { amplitude: 13, period: 16.9 },
        { amplitude: 5, period: 6.7 },
      ],
    },
    sway: { amplitude: 30, minSpeed: 0.4, maxSpeed: 1 },
    spin: { minSpeed: -1.1, maxSpeed: 1.1 },
    /** 翻到侧面时留一点宽度，否则花瓣会整帧消失，看起来像闪烁 */
    flip: { minSpeed: 0.5, maxSpeed: 1.5, minWidth: 0.16 },
    fade: { start: 0.7, endMultiplier: 0.1 },
    perMegapixel: 30,
    maxCount: 160,
  },
  sprite: {
    kind: "petal",
    hues: [322, 326, 330, 334],
    lightFrom: 88,
    lightTo: 72,
    glow: "rgba(255, 183, 205, 0.6)",
    glowBlur: 10,
  },
};

/**
 * 雪暴的调参表。
 *
 * 与雪共用同一套渲染场（hooks/useFallingField.ts），差别全在数值：
 * 雪片更小更密、落得更快、被风横着抽过去。雪是「飘」，雪暴是「刮」，
 * 所以横摆反而要收小 —— 风已经把所有轨迹拧向同一个方向，
 * 再叠大幅度的正弦横摆只会把那股劲卸掉。
 */

import type { FallingEffect } from "../types/falling";

export const blizzard: FallingEffect = {
  physics: {
    layers: [
      { share: 0.42, zMin: 0.3, zMax: 0.5, alpha: 0.42 },
      { share: 0.34, zMin: 0.5, zMax: 0.75, alpha: 0.56 },
      { share: 0.24, zMin: 0.75, zMax: 1, alpha: 0.72 },
    ],

    /** 落速约为雪的 2.7 倍。雪暴的雪片是被抽下来的，不是飘下来的 */
    speedAtZ1: 165,

    /** 雪片比雪的更小：暴雪里看不清单片，看到的是一片白 */
    sizeAtZ1: 20,

    /**
     * 横向风约为雪的 14 倍。这个值决定了雪暴的全部观感 ——
     * 与落速配起来约 45 度，雪几乎是横着飞过画面的。
     */
    wind: {
      base: 170,
      gust: [
        { amplitude: 62, period: 11.3 },
        { amplitude: 28, period: 4.7 },
      ],
    },

    /** 横摆只留一点不规则，免得轨迹整齐得像印刷 */
    sway: { amplitude: 16, minSpeed: 0.6, maxSpeed: 1.4 },

    /** 翻得比雪快，小尺寸下这点自转是「雪片在打旋」的唯一线索 */
    spin: { minSpeed: -1.3, maxSpeed: 1.3 },

    /** 淡出点比雪更靠下：雪暴的雪片该一直冲到画面底边 */
    fade: { start: 0.78, endMultiplier: 0.35 },

    /** 雪的 5 倍多。雪暴的观感几乎全靠密度 —— 静帧里看不出速度，只看得见有多少 */
    perMegapixel: 320,
    maxCount: 1400,
  },
  sprite: {
    kind: "glyph",
    chars: ["❄", "❅", "❆", "•", "·"],
    color: "#ffffff",
    glow: "rgba(230, 242, 255, 0.7)",
    glowBlur: 12,
    font: '"Segoe UI Symbol", "Noto Sans Symbols 2", "Apple Symbols", sans-serif',
  },
};

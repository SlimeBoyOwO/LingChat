/**
 * 下落类粒子（雪、樱花）共用的类型。
 *
 * 雪和樱花只有形态与配色不同，物理完全一致，所以共用一套渲染场，
 * 差异全部收敛在 FallingEffect 里：physics 是数值，sprite 描述预渲染的形态。
 */

/** 一层粒子的景深参数。远小慢暗、近大快亮，视差由 z 推导，不必给每层凑一组数字 */
export interface FallingLayerOption {
  /** 该层粒子占总数比例 */
  share: number;
  /** 深度区间，1 表示贴脸 */
  zMin: number;
  zMax: number;
  /** 该层粒子的透明度 */
  alpha: number;
}

/** 水平风：一条基准风加两条周期互质的正弦阵风 */
export interface FallingWind {
  base: number;
  gust: { amplitude: number; period: number }[];
}

/** 逐粒正弦横摆，雪飘起来的观感主要来自它 */
export interface FallingSway {
  /** 横摆振幅，CSS 像素，按深度缩放 */
  amplitude: number;
  /** 横摆角速度区间，弧度每秒 */
  minSpeed: number;
  maxSpeed: number;
}

/** 自转角速度区间，弧度每秒，允许负值即允许反转 */
export interface FallingSpin {
  minSpeed: number;
  maxSpeed: number;
}

/** 花瓣翻滚：用横向缩放的正弦振荡伪造三维翻面 */
export interface FallingFlip {
  /** 翻面角速度区间，弧度每秒 */
  minSpeed: number;
  maxSpeed: number;
  /** 翻到侧面时的最小宽度比例，取 0 会让花瓣瞬间消失一帧 */
  minWidth: number;
}

/** 接近画面底部时逐渐淡出 */
export interface FallingFade {
  /** 开始淡出的位置，画布高度的比例 */
  start: number;
  /** 画布底边处剩余的透明度倍率 */
  endMultiplier: number;
}

/** 下落粒子的物理参数。同一个深度 z 同时决定尺寸、落速与横摆幅度 */
export interface FallingPhysics {
  layers: FallingLayerOption[];
  /** 深度为 1 时的下落速度，像素每秒 */
  speedAtZ1: number;
  /** 深度为 1 时的粒子直径，CSS 像素 */
  sizeAtZ1: number;
  wind: FallingWind;
  sway: FallingSway;
  spin: FallingSpin;
  /** 雪花不需要翻面，留空即可 */
  flip?: FallingFlip;
  fade: FallingFade;
  /** 每百万像素的粒子数，密度随画布面积缩放，超宽屏不会显得稀 */
  perMegapixel: number;
  /** 粒子总数上限 */
  maxCount: number;
}

/** 雪花：预渲染成精灵的字形表 */
export interface GlyphSpriteSpec {
  kind: "glyph";
  /** 每个字形预渲染成一张精灵，粒子随机取用 */
  chars: string[];
  color: string;
  /** 光晕颜色，对应旧实现里的 text-shadow */
  glow: string;
  /** 光晕模糊半径，精灵像素 */
  glowBlur: number;
  /** 字形字体栈 */
  font: string;
}

/** 樱花：预渲染成精灵的花瓣，每个色相一张 */
export interface PetalSpriteSpec {
  kind: "petal";
  hues: number[];
  /** 渐变两端的明度，百分比 */
  lightFrom: number;
  lightTo: number;
  glow: string;
  glowBlur: number;
}

export type SpriteSpec = GlyphSpriteSpec | PetalSpriteSpec;

/** 一个特效的完整描述：物理参数加形态 */
export interface FallingEffect {
  physics: FallingPhysics;
  sprite: SpriteSpec;
}

/** 组件的属性契约 */
export interface FallingProps {
  enabled?: boolean;
  /** 密度倍率，0 到 2 */
  intensity?: number;
}

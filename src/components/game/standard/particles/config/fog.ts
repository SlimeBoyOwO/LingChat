/**
 * 雾的调参表。
 *
 * 雾做成了一团团缓慢横移的云影，而不是一张盖满全屏的噪声图。
 * 后者铺满屏、对比低、又没有可辨认的形体，眼睛抓不住任何东西 ——
 * 上一版就是这么做的，结果画面只是「变白了」，看不出有雾在动。
 *
 * 云影是离散的形体：哪怕很淡，眼睛也会跟着它走。动态感来自能被追踪的对象，
 * 不来自整体透明度。代价是不能再用铺图那种方式覆盖全屏，
 * 只能一团一团画，所以数量必须克制 —— 全透明的像素一样要参与混合。
 */

/** 云形的种类数。同屏的云团从这里随机取形，免得一眼看出是同一朵在重复 */
export const WISP_VARIANTS = 6;

/** 单张云影精灵的尺寸，设备像素。画出去时会缩放到屏幕尺度，雾本来就经得起放大 */
export const SPRITE_WIDTH = 384;
export const SPRITE_HEIGHT = 162;

export interface WispLayerOption {
  /** 云团宽度占画布宽度的比例区间 */
  minWidthRatio: number;
  maxWidthRatio: number;
  /** 同屏数量：每百万 CSS 像素几团，超宽屏才不会显得稀 */
  perMegapixel: number;
  maxCount: number;
  /** 横向漂移速度区间，像素每秒 */
  minSpeed: number;
  maxSpeed: number;
  /** 透明度区间 */
  minAlpha: number;
  maxAlpha: number;
  /** 纵向位置区间，占画布高度的比例 */
  minY: number;
  maxY: number;
}

/**
 * 由远及近三层。远层小、慢、淡、位置高，近层大、快、浓、位置低 ——
 * 大气透视和地面起雾两件事同时由这一条规则表达。
 *
 * 透明度刻意都压得很低：要的是「有一点雾」，不是把背景盖住。
 */
export const WISP_LAYERS: WispLayerOption[] = [
  {
    minWidthRatio: 0.3,
    maxWidthRatio: 0.5,
    perMegapixel: 1.9,
    maxCount: 7,
    minSpeed: 5,
    maxSpeed: 11,
    minAlpha: 0.14,
    maxAlpha: 0.22,
    minY: 0.02,
    maxY: 0.5,
  },
  {
    minWidthRatio: 0.38,
    maxWidthRatio: 0.62,
    perMegapixel: 2.1,
    maxCount: 8,
    minSpeed: 13,
    maxSpeed: 26,
    minAlpha: 0.17,
    maxAlpha: 0.26,
    minY: 0.2,
    maxY: 0.68,
  },
  {
    minWidthRatio: 0.45,
    maxWidthRatio: 0.75,
    perMegapixel: 1.9,
    maxCount: 7,
    minSpeed: 30,
    maxSpeed: 58,
    minAlpha: 0.21,
    maxAlpha: 0.32,
    minY: 0.42,
    maxY: 0.96,
  },
];

/**
 * 云的颜色。取中性灰而不是纯白：雾是空气里的悬浮物，
 * 该在暗背景上提亮、在亮背景上压暗，中间调才两头都成立。
 */
export const WISP_COLOR = "158, 168, 186";

/**
 * 透明度的呼吸。每朵云按各自的相位在基准值上下浮动，
 * 这是「活的」和「贴图在滑」的分界，所以周期不能太短，幅度也不能太大。
 */
export const BREATH_PERIOD = 4.6;
export const BREATH_AMPLITUDE = 0.22;

/** 上下浮动，幅度按云团自身高度算。只横移会像贴在玻璃上滑动 */
export const BOB_PERIOD = 9.3;
export const BOB_AMPLITUDE = 0.07;

/** 画布后备缓冲的像素上限。雾最经得起降分辨率，糊一点看不出来 */
export const MAX_BACKING_PIXELS = 1_200_000;

export const MAX_SCALE = 2;

/** 缩放下限。雾糊一点看不出来，压低能省下可观的填充率 */
export const MIN_SCALE = 0.6;

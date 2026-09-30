/**
 * 下落类粒子（雪、樱花）共用的引擎常量。
 *
 * 这里只放与具体形态无关的数值：后备缓冲预算、平铺数组的余量、精灵的尺寸约定。
 * 「雪该多大、樱花该多快」这类观感参数在 snow.ts / sakura.ts 里。
 */

/** 画布后备缓冲的像素上限。两者都是柔光粒子，没必要按屏幕原生分辨率渲染。 */
export const MAX_BACKING_PIXELS = 2_200_000;

/** 后备缓冲的最大缩放倍率，三倍屏也不会超过它 */
export const MAX_SCALE = 2;

/** 后备缓冲的最小缩放倍率，再低就糊得看不出形状了 */
export const MIN_SCALE = 0.75;

/** 单帧最大步长，秒。切后台再回来时不让粒子瞬移一大截 */
export const MAX_STEP = 0.05;

/** 预渲染精灵的边长，设备像素。粒子最大约 32 CSS 像素，倍率 2 时仍在它的采样范围内 */
export const SPRITE_CELL = 96;

/** 精灵四周留给光晕的空白，设备像素 */
export const SPRITE_GLOW_PAD = 14;

/** 精灵内容（字形或花瓣）的外接圆半径，设备像素 */
export const SPRITE_RADIUS = SPRITE_CELL / 2 - SPRITE_GLOW_PAD;

/** 绘制四边形相对粒子直径的放大倍率，把光晕的留白一并算进去 */
export const SPRITE_PAD = SPRITE_CELL / (SPRITE_RADIUS * 2);

/** 横向环绕边距，像素。风把粒子吹向一侧，靠环绕维持分布均匀 */
export const WRAP_MARGIN = 120;

/** 生成时落在画布上方多少像素内，让粒子从画面外飘入而不是凭空出现 */
export const SPAWN_BAND = 160;

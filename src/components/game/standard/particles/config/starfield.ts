/**
 * 星空的调参表。观感参数集中在这里，改数值就能调星空，不必动引擎。
 *
 * 星点活在一个真实的三维空间里：每颗星有固定的世界坐标与深度，镜头沿一条
 * 缓慢摆动的方向平移，屏幕坐标由透视除法算出。于是近处的星又大又快又亮、
 * 远处的星又小又慢又暗，三个比例由同一个深度倒数决定，不用分别凑数字。
 */

/**
 * starCount 的定义面积（1600 乘 900）。
 *
 * 星数按画布面积等比缩放，疏密才不随窗口大小变：塞进桌宠那个小头像框时，
 * 200 颗星会挤成一团白雾，按面积缩下来才跟全屏时是同一种疏密。
 * 代价是 starCount 变成了「定义面积上的星数」，1080p 全屏实际约 1.4 倍，
 * 1366 乘 768 约 0.7 倍，改这个常量就能换基准。
 */
export const REFERENCE_AREA = 1600 * 900;

/** 星数下限。再小的容器也留得住几颗，不至于空成一片黑。 */
export const MIN_STARS = 16;

/** 星数上限。4K 全屏按面积算会到上千颗，这里压住。 */
export const MAX_STARS = 900;

/** 焦距相对画布短边的比例。调小视场变广、纵深更夸张，调大则接近平面。 */
export const FOCAL_RATIO = 0.9;

/**
 * scrollSpeed 为 1 时，深度为 1 的星点每秒在屏幕上走过的像素。
 *
 * 镜头走的是世界距离，换算成像素要乘焦距除深度，所以速度不随窗口大小变，
 * 只有深度带来的快慢差会留下。
 */
export const SCREEN_SPEED_PER_UNIT = 30;

/**
 * 拖尾的曝光时间，秒。星点速度乘它得到拖尾长度。
 *
 * 默认速度下这一截不到一个像素，看着仍是圆点；把 scrollSpeed 调大就会拉出丝，
 * 和雨的拖尾是同一个物理关系。
 */
export const SHUTTER_SECONDS = 0.06;

/**
 * 漂移方向沿正弦缓慢来回摆动，免得全场朝一个方向推到天荒地老。
 *
 * amplitude 用弧度，Math.PI 就是整圈来回扫。
 */
export const DRIFT = { amplitude: Math.PI, periodSeconds: 240 };

/** 逐粒闪烁的相位循环周期，秒 */
export const TWINKLE_PERIOD = 3.6;

/** 亮度下限，闪烁时最暗到原亮度的这个比例 */
export const TWINKLE_MIN = 0.28;

/** 亮度上限 */
export const TWINKLE_MAX = 1;

/**
 * 亮度量化档数。
 *
 * 批绘制要求同批同透明度，连续亮度只能先量化成有限几档，一键一档地各画一次。
 * 档数越多过渡越顺，绘制调用也越多；三星点大小下 3 档已经看不出台阶。
 */
export const TWINKLE_LEVELS = 3;

/** 各档相对本层透明度的系数，数量与 TWINKLE_LEVELS 对应 */
export const TWINKLE_GAIN = [0.45, 0.72, 1];

/**
 * 归位判定与投胎的边距，按画布短边的比例给，并夹在上下限内。
 *
 * 写成绝对像素的话，同一份参数铺满全屏和塞进桌宠头像框里的占比会差一个数量级，
 * 小容器里会有一大半星点耗在画外飘进来的路上。归位边距是「确认已经出画」的门槛，
 * 投胎边距取它的两倍：够把新星藏在画面外沿，又不用在外面飞太久。
 */
export const MARGIN = { ratio: 0.05, min: 8, max: 32 };

export interface StarLayerOption {
  /** 该层星点占总数比例 */
  share: number;
  /** 深度区间，1 是远景基准面，越小越贴近镜头 */
  zMin: number;
  zMax: number;
  /** 星点直径，CSS 像素 */
  width: number;
  /** 星点本体的透明度 */
  alpha: number;
}

/**
 * 由远及近三层，顺序即绘制顺序。
 *
 * width 与 alpha 都跟着各层中位深度的倒数走（约为 2 乘 0.37 除以中位深度），
 * 所以近景不只是更大，也更快更亮，三者是一套数。逐颗深度还是层内随机的，
 * 层内因此仍有大小与速度的细微参差，不会看着像印刷品。
 */
export const STAR_LAYERS: StarLayerOption[] = [
  { share: 0.52, zMin: 0.62, zMax: 1, width: 0.9, alpha: 0.52 },
  { share: 0.31, zMin: 0.4, zMax: 0.62, width: 1.4, alpha: 0.66 },
  { share: 0.17, zMin: 0.22, zMax: 0.4, width: 2.3, alpha: 0.8 },
];

/**
 * 光晕遍。同一批星点只用一条路径，两遍各描一次：
 * 第一遍是星点本体，第二遍把同一路径加宽压暗，叠出一点辉光。
 * 两遍是加法关系，先后无所谓。
 */
export const GLOW_PASSES = [
  { widthScale: 1, alphaScale: 1 },
  { widthScale: 2.6, alphaScale: 0.16 },
];

/**
 * 画布后备缓冲的像素上限。
 *
 * 倍率取设备像素比、上限与这个预算三者的最小值，于是高分屏上不会真按两倍渲染：
 * 1080p 两倍屏会落在 1.3 倍上下，既比原来按 CSS 像素画清楚，又不必为一块全屏
 * 透明画布付原生分辨率的清屏与合成开销。画布本身超过预算时由 MIN_SCALE 兜底。
 */
export const MAX_BACKING_PIXELS = 3_500_000;

/** 后备缓冲的最大缩放倍率，三倍屏也不会超过它 */
export const MAX_SCALE = 2;

/**
 * 后备缓冲的最小缩放倍率。
 *
 * 雨是柔和的，可以降到 1 以下；星点是硬边小亮点，描边宽度本来就只有一个像素左右，
 * 再降采样会让亮度随亚像素位置忽明忽暗，所以这里守住一比一，只许往上加。
 */
export const MIN_SCALE = 1;

/** colors 传了空数组或全是不认识的颜色时的兜底调色板 */
export const DEFAULT_COLORS = [
  "rgb(173, 216, 230)",
  "rgb(176, 224, 230)",
  "rgb(241, 141, 252)",
  "rgb(176, 230, 224)",
  "rgb(173, 230, 216)",
];

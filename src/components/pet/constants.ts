/**
 * 桌宠几何预算（CSS px，进桌宠模式时整体乘 `pet.scale`）。
 *
 * 双窗口：宠物窗是**锚点**（尺寸与位置都不因气泡变化），气泡窗跟随它。
 * **两窗同宽** —— 视觉上是一个整体，宽度不一致会明显错位。
 *
 * ⚠️ 这里与 `src-tauri/src/api/pet.rs` 的窗口尺寸是一对必须同步的常量。
 */

/** 两窗共用的宽度：头像圆框 210 + 两侧各 27 的呼吸边 */
export const WINDOW_WIDTH_BASE = 264;

/** 头像带 */
export const AVATAR_BAND_BASE = 210;

/** 输入带 */
export const CHAT_BASE_H = 70;

/** 宠物窗高度：只有头像与输入框 —— 顶边即头像顶边，因此天然贴屏幕顶 */
export const PET_WINDOW_H_BASE = AVATAR_BAND_BASE + CHAT_BASE_H;

/** 气泡与宠物头顶之间的间隙 */
export const BUBBLE_GAP_BASE = 8;

/** 气泡带：above 模式下气泡窗里留给气泡的整块高度 */
export const BAND_BASE = 278;

/** 气泡长尾伸出气泡盒底边的距离，气泡窗底部必须预留这么多，否则尾巴被窗口裁掉 */
export const TAIL_OVERHANG_BASE = 10;

/** 气泡窗高度：气泡带 + 长尾余量 + 与宠物头顶的间隙 */
export const BUBBLE_WINDOW_H_BASE = BAND_BASE + TAIL_OVERHANG_BASE + BUBBLE_GAP_BASE;

/** 气泡宽度 = 窗口宽 × 该比例；两侧留白要容得下气泡的圆角与投影 */
export const DIALOG_WIDTH_RATIO = 0.88;

/** 通知条高度上限；钉在气泡带顶端，不影响气泡位置 */
export const NOTIFICATION_MAX_BASE = 72;

/** 气泡正文可达高度 */
export const DIALOG_MAX_BASE = 190;

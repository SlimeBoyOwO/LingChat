/** 窗口宽：头像圆框 210 + 两侧各 15 呼吸边（工具按钮/光晕都在里面） */
export const PET_WIDTH_BASE = 240;
/** 头像带高 = 可见圆框高度：圆框顶边就是窗口顶边，系统钳制因此等价于“宠物贴屏幕上边缘” */
export const AVATAR_BAND_BASE = 210;
/** 输入带高：给足 2 行输入（窗口尺寸固定，多行不再改窗口） */
export const CHAT_BASE_H = 70;
/** 气泡/通知带预算高：决定窗口高度与气泡最大高度；带子实际高度随内容，余量交给底部吸收带。
    三条带之和必须与 src-tauri/src/api/pet.rs 的窗口高度一致 */
export const DIALOG_MAX_BASE = 200;

/**
 * 手机悬浮窗的窗口宽度（占屏幕宽度的比例）。
 *
 * 与桌面端固定 240dp 不同：手机屏幕宽度差异很大（360–430dp），
 * 固定 dp 会让桌宠在小屏上占到 60% 以上，显得巨大。按屏幕比例算
 * 才能保证视觉占比一致。
 *
 * 必须与 Kotlin 侧 `COLLAPSED_WIDTH_RATIO` / `EXPANDED_WIDTH_RATIO` 保持一致
 * （见 `android/.../FloatingPetPlugin.kt`）。
 */
export const MOBILE_COLLAPSED_WIDTH_RATIO = 1 / 6;
export const MOBILE_EXPANDED_WIDTH_RATIO = 2 / 5;

/**
 * 手机悬浮窗的高宽比。
 *
 * - 收起态 1.15：只显示头像，略高一点给下方提示文字留空间
 * - 展开态 2.0：头像 + 气泡 + 输入框，与桌面端窗口比例一致
 *
 * 必须与 Kotlin 侧 `COLLAPSED_HEIGHT_RATIO` / `EXPANDED_HEIGHT_RATIO` 一致。
 */
export const MOBILE_COLLAPSED_HEIGHT_RATIO = 1.15;
export const MOBILE_EXPANDED_HEIGHT_RATIO = 2.0;

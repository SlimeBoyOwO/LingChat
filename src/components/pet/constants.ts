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
 * ## 悬浮窗是「固定逻辑画布 + 整体等比缩放」
 *
 * 悬浮窗里的页面**不做响应式布局**：始终按上面那套桌面端尺寸
 * （240dp 宽）排版，再由 `PetMode.vue` 的 `transform: scale(窗口宽度 / 240)`
 * 缩放到窗口大小。这样布局只有一套、内容恰好铺满画布，窗口里不会
 * 出现大块透明区（悬浮窗没有逐像素穿透，空白区域会吃掉下层 App 的触摸）。
 *
 * 代价：文字绝对大小与窗口宽度成正比，所以展开态不能太窄——
 * 2/5 屏宽时缩放系数只有 0.6，15px 的字缩到 9px 就看不清了。
 *
 * 必须与 Kotlin 侧 `COLLAPSED_WIDTH_RATIO` / `EXPANDED_WIDTH_RATIO` 保持一致
 * （见 `android/.../FloatingPetPlugin.kt`）。
 */
export const MOBILE_COLLAPSED_WIDTH_RATIO = 1 / 6;
export const MOBILE_EXPANDED_WIDTH_RATIO = 0.6;

/**
 * 手机悬浮窗的高宽比 —— 直接由逻辑画布尺寸推出。
 *
 * - 收起态 = 头像带 210 → 210/240 = 0.875
 * - 展开态 = 头像带 + 输入带 = 280 → 280/240 ≈ 1.1667
 *
 * 气泡出现时内容会变高，窗口高度由前端通过 `resizeFloatingPet` 再撑高，
 * 不在这里预留。
 *
 * 必须与 Kotlin 侧 `COLLAPSED_HEIGHT_RATIO` / `EXPANDED_HEIGHT_RATIO` 一致。
 */
export const MOBILE_COLLAPSED_HEIGHT_RATIO = AVATAR_BAND_BASE / PET_WIDTH_BASE;
export const MOBILE_EXPANDED_HEIGHT_RATIO = (AVATAR_BAND_BASE + CHAT_BASE_H) / PET_WIDTH_BASE;

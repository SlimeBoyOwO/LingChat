/**
 * **桌面端**窗口宽：头像圆框 210 + 两侧各 15 呼吸边（工具按钮/光晕都在里面）。
 *
 * ⚠️ 那 15px 呼吸边是**桌面端专属**的：设置/自动/返回/截图四个按钮挂在
 * 头像左外侧（`-left-3.5`），光晕也溢出头像圆框。悬浮窗里这排按钮被
 * `v-if="!floatingMode"` 全部隐藏，呼吸边就变成了**纯透明的空白**——
 * 而 Android 悬浮窗没有逐像素穿透，那片空白既难看又会吃掉下层 App 的触摸。
 *
 * 因此悬浮窗用的逻辑画布宽度是 {@link FLOATING_LOGICAL_WIDTH}，不是这个值。
 */
export const PET_WIDTH_BASE = 240;
/** 头像带高 = 可见圆框高度：圆框顶边就是窗口顶边，系统钳制因此等价于“宠物贴屏幕上边缘” */
export const AVATAR_BAND_BASE = 210;
/** 输入带高：给足 2 行输入（窗口尺寸固定，多行不再改窗口） */
export const CHAT_BASE_H = 70;
/** 气泡/通知带预算高：决定窗口高度与气泡最大高度；带子实际高度随内容，余量交给底部吸收带。
    三条带之和必须与 src-tauri/src/api/pet.rs 的窗口高度一致 */
export const DIALOG_MAX_BASE = 200;

/**
 * 手机悬浮窗的逻辑画布宽度（dp）。
 *
 * ## 为什么不复用 PET_WIDTH_BASE(240)
 *
 * 240 = 头像圆框 210 + 两侧各 15 呼吸边，而呼吸边只服务桌面端那排悬停按钮
 * 与光晕（见 {@link PET_WIDTH_BASE}）。悬浮窗里按钮全被隐藏 → 那 15px 变成
 * 纯透明带，**头像被水平居中，左右各留 15 逻辑 px 的空白**。
 *
 * 真机实测（诊断浮层的原始数字，窗口 216×252）：
 *
 * ```
 * canvas=216x252@0,0      ← 画布恰好铺满窗口
 * avatar=189x189@14,0     ← 189 = 210×0.9，x=14 ≈ (216-189)/2 = 13.5
 * ```
 *
 * 画布没问题、窗口也没问题，**多出来的就是这圈空白**。而且它的绝对宽度
 * 随窗口放大：收起态窗口 60dp、fit=0.25 → 单侧只有 3.75px（看不见）；
 * 展开态窗口 216dp、fit=0.9 → 单侧 13.5px（约 2.9mm，一眼可见）。
 * 这正是用户那句「缩小的时候正常，放大就不正常」的来源——**占比恒定
 * 12.5%，绝对宽度却差 3.6 倍**。
 *
 * 取 210 后逻辑画布宽度 = 头像带宽度：头像铺满画布、缩放系数由
 * `窗口宽 / 210` 给出，空白从根上消失，宠物同时放大约 14%（文字更好读）。
 *
 * ⚠️ 必须与 Kotlin 侧 `FLOATING_LOGICAL_WIDTH` 逐位一致——窗口高度、
 * `currentScale()`、`status.scale` 三处都由它换算（见 FloatingPetPlugin.kt）。
 */
export const FLOATING_LOGICAL_WIDTH = AVATAR_BAND_BASE;

/**
 * 手机悬浮窗的窗口宽度（占屏幕宽度的比例）。
 *
 * ## 悬浮窗是「固定逻辑画布 + 整体等比缩放」
 *
 * 悬浮窗里的页面**不做响应式布局**：始终按 {@link FLOATING_LOGICAL_WIDTH}
 * （210dp 宽）排版，再由 `PetMode.vue` 的
 * `transform: scale(窗口宽度 / 210)` 缩放到窗口大小。这样布局只有一套、
 * 内容恰好铺满画布，窗口里不会出现透明区（悬浮窗没有逐像素穿透，
 * 空白区域会吃掉下层 App 的触摸）。
 *
 * 代价：文字绝对大小与窗口宽度成正比，所以展开态不能太窄——
 * 展开态取 3/5 屏宽，缩放系数约 1.03，15px 的字渲染成约 15.4px。
 *
 * 必须与 Kotlin 侧 `COLLAPSED_WIDTH_RATIO` / `EXPANDED_WIDTH_RATIO` 保持一致
 * （见 `android/.../FloatingPetPlugin.kt`）。
 */
export const MOBILE_COLLAPSED_WIDTH_RATIO = 1 / 6;
export const MOBILE_EXPANDED_WIDTH_RATIO = 0.6;

/**
 * 手机悬浮窗的高宽比 —— 直接由逻辑画布尺寸推出。
 *
 * - 收起态 = 头像带 210 → 210/210 = 1（正方形，与头像圆框严丝合缝）
 * - 展开态 = 头像带 + 输入带 = 280 → 280/210 ≈ 1.3333
 *
 * 气泡出现时内容会变高，窗口高度由前端通过 `resizeFloatingPet` 再撑高，
 * 不在这里预留。
 *
 * 必须与 Kotlin 侧 `COLLAPSED_HEIGHT_RATIO` / `EXPANDED_HEIGHT_RATIO` 一致。
 */
export const MOBILE_COLLAPSED_HEIGHT_RATIO = AVATAR_BAND_BASE / FLOATING_LOGICAL_WIDTH;
export const MOBILE_EXPANDED_HEIGHT_RATIO =
  (AVATAR_BAND_BASE + CHAT_BASE_H) / FLOATING_LOGICAL_WIDTH;

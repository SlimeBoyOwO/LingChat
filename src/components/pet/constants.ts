// 桌宠几何预算（CSS px，进桌宠模式整体乘 pet.scale）；窗口尺寸必须与 src-tauri/src/api/pet.rs 同步

// 两窗共用宽度：头像圆框 210 + 两侧各 27 的呼吸边（两窗同宽，视觉上是一个整体）
export const WINDOW_WIDTH_BASE = 264;

export const AVATAR_BAND_BASE = 210;

// 输入带高：装得下 2 行输入框且底部不留空（输入行靠下对齐，余量落在头像侧）
export const CHAT_BASE_H = 64;

// 输入行距窗口底边的呼吸量（给发送键光晕留地方，不参与气泡定位）
export const CHAT_BASE_PB = 6;

// ─── 频谱圆环（桌宠头像外圈）─────────────────────────────
// 圆环要画在头像圆框外侧，但框外原本只有左右各 27 的呼吸边，上方与窗口顶边齐平、一点余量都没有。
// 因此在窗口顶部加一段留白专供圆环，头像尺寸与外观不变。
// 圆心 = (WINDOW_WIDTH_BASE / 2, PET_SPECTRUM_TOP_PAD_BASE + AVATAR_BAND_BASE / 2)，头像半径 105。
export const PET_SPECTRUM_TOP_PAD_BASE = 26;
// 基准半径：紧贴头像圆框外沿（半径 105），再往里 1px 让圆头柱底与框沿之间不留亮缝
export const PET_SPECTRUM_R0_BASE = 104;
// 单根柱最大长度（受窗口上沿限制：r0 + 这个值须 ≤ 105 + 顶部留白，也 ≤ 窗口半宽 132）
export const PET_SPECTRUM_LEN_BASE = 24;
// 频带数与柱数（一一对应）：铺得比主界面密，绕满一圈
export const PET_SPECTRUM_BARS = 96;
// 柱宽占相邻柱间距的比例：1 = 相邻柱相接，整圈不留缝
export const PET_SPECTRUM_BAR_RATIO = 1;
// 辉光（canvas shadowBlur）：每根柱都要多一遍带模糊的填充，关掉最省
export const PET_SPECTRUM_GLOW_BASE = 0;
// 包络跟随速度：比主界面（上升立刻到位、下落 0.18）慢一档，
// 圆环才是缓缓起伏而不是跟着鼓点一下窜到顶
export const PET_SPECTRUM_SMOOTHING = { rise: 0.25, fall: 0.07 };
// 最外缘：128，须 ≤ 132（窗口半宽）且 ≤ 105 + 顶部留白
const PET_SPECTRUM_OUTER_BASE =
  PET_SPECTRUM_R0_BASE + PET_SPECTRUM_LEN_BASE + PET_SPECTRUM_GLOW_BASE;
// 画布边长：最外缘再各留 3px 余量，居中后 x∈[1,263]、y∈[0,262]，都在窗口内
export const PET_SPECTRUM_CANVAS_BASE = 2 * (PET_SPECTRUM_OUTER_BASE + 3);

// 宠物窗高 = 顶部频谱留白 + 头像带 + 输入带
// （顶边不再是头像顶边，气泡定位要按留白补偿，见 pet.rs 的 place_bubble）
export const PET_WINDOW_H_BASE = PET_SPECTRUM_TOP_PAD_BASE + AVATAR_BAND_BASE + CHAT_BASE_H;

// 气泡与宠物头顶的间隙
export const BUBBLE_GAP_BASE = 8;

// 气泡带：above 模式下留给气泡的整块高度
export const BAND_BASE = 278;

// 长尾伸出气泡盒底边的距离（气泡窗底部须预留，否则尾巴被裁）
export const TAIL_OVERHANG_BASE = 10;

// 气泡窗高 = 气泡带 + 长尾 + 间隙
export const BUBBLE_WINDOW_H_BASE = BAND_BASE + TAIL_OVERHANG_BASE + BUBBLE_GAP_BASE;

// 气泡宽度 = 窗口宽 × 该比例（两侧留给圆角与投影）
export const DIALOG_WIDTH_RATIO = 0.88;

// 通知条高度上限（钉在气泡带顶端，不影响气泡位置）
export const NOTIFICATION_MAX_BASE = 72;

// 气泡正文可达高度（超出在盒内滚动）
export const DIALOG_MAX_BASE = 190;

/**
 * 手机悬浮窗的逻辑画布宽度（dp）。
 *
 * ## 为什么不复用 WINDOW_WIDTH_BASE(264)
 *
 * 264 = 头像圆框 210 + 两侧各 27 呼吸边，而呼吸边只服务桌面端那排悬停按钮
 * 与光晕（见 {@link WINDOW_WIDTH_BASE}）。悬浮窗里按钮全被隐藏 → 那 27px 变成
 * 纯透明带，**头像被水平居中，左右各留 27 逻辑 px 的空白**。
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
 * - 展开态 = 头像带 + 输入带 = 274 → 274/210 ≈ 1.3048
 *
 * 气泡出现时内容会变高，窗口高度由前端通过 `resizeFloatingPet` 再撑高，
 * 不在这里预留。
 *
 * ⚠️ 输入带高被上游从 70 改成 64（并新增 6px 底部呼吸量），展开态高宽比
 * 因此从 280/210 变成 274/210。**Kotlin 侧的 `EXPANDED_HEIGHT_RATIO`
 * 必须同步改成同一个比值**，否则前端画布与原生窗口对不上，展开态会
 * 露出或裁掉 6dp。
 *
 * 必须与 Kotlin 侧 `COLLAPSED_HEIGHT_RATIO` / `EXPANDED_HEIGHT_RATIO` 一致。
 */
export const MOBILE_COLLAPSED_HEIGHT_RATIO = AVATAR_BAND_BASE / FLOATING_LOGICAL_WIDTH;
export const MOBILE_EXPANDED_HEIGHT_RATIO = PET_WINDOW_H_BASE / FLOATING_LOGICAL_WIDTH;

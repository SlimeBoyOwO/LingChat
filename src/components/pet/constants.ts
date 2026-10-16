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

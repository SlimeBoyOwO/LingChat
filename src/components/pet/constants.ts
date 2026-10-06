// 桌宠几何预算（CSS px，进桌宠模式整体乘 pet.scale）；窗口尺寸必须与 src-tauri/src/api/pet.rs 同步

// 两窗共用宽度：头像圆框 210 + 两侧各 27 的呼吸边（两窗同宽，视觉上是一个整体）
export const WINDOW_WIDTH_BASE = 264;

export const AVATAR_BAND_BASE = 210;

// 输入带高：装得下 2 行输入框且底部不留空（输入行靠下对齐，余量落在头像侧）
export const CHAT_BASE_H = 64;

// 输入行距窗口底边的呼吸量（给发送键光晕留地方，不参与气泡定位）
export const CHAT_BASE_PB = 6;

// 宠物窗高 = 头像带 + 输入带（顶边即头像顶边，因此能贴屏幕顶）
export const PET_WINDOW_H_BASE = AVATAR_BAND_BASE + CHAT_BASE_H;

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

/** 窗口宽：头像圆框 210 + 两侧各 15 呼吸边（工具按钮/光晕都在里面） */
export const PET_WIDTH_BASE = 240;
/** 头像带高 = 可见圆框高度：圆框顶边就是窗口顶边，系统钳制因此等价于“宠物贴屏幕上边缘” */
export const AVATAR_BAND_BASE = 210;
/** 输入带高：给足 2 行输入（窗口尺寸固定，多行不再改窗口） */
export const CHAT_BASE_H = 70;
/** 气泡/通知带预算高：决定窗口高度与气泡最大高度；带子实际高度随内容，余量交给底部吸收带。
    三条带之和必须与 src-tauri/src/api/pet.rs 的窗口高度一致 */
export const DIALOG_MAX_BASE = 200;

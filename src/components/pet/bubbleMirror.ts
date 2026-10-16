export interface BubbleMirror {
  status: string;
  line: string;
  /** 台词序号，由宠物窗单调发号；气泡窗在整句显示完后原样回传（PET_LINE_DRAINED） */
  lineId: number;
  title: string;
  subtitle: string;
  emotion: string;
  motionText: string;
  avatarAudio: string;
  textSpeed: number;
  petScale: number;
  bubbleSide: BubbleSide;
  bubbleAlign: BubbleAlign;
  alignInset: number;
  swapping: boolean;
  notification: { isVisible: boolean; title: string; message: string; type: string };
}

export type BubbleSide = "above" | "below" | "left" | "right";
export type BubbleAlign = "top" | "bottom";

export const PET_BUBBLE_EVENT = "pet:bubble-mirror";
export const PET_BUBBLE_REQUEST = "pet:bubble-request";

/** 台词序号计数器。模块级：宠物窗重挂载后不回退，不会与在途的旧号撞车 */
let lineSeq = 0;
export const nextLineId = () => ++lineSeq;

export interface LineDrainedPayload {
  lineId: number;
}

/**
 * 气泡窗 → 宠物窗：这一句台词已经完整显示出来了。
 *
 * 每句恰好一次、显式发射，且过期渲染会被 DialogueBox 直接丢弃——因此宠物窗可以
 * 用「回报过的号 === 当前号」本地判断打字是否结束，不必跨窗口读一个变来变去的布尔量。
 * 只有真的显示完整才算数：整段复现、自然打完、点击补全都算，隐藏气泡不算。
 */
export const PET_LINE_DRAINED = "pet:line-drained";

/** 宠物窗 → 气泡窗：补全当前打字动画（点击跳过：先补全文本，不推进队列） */
export const PET_FINISH_TYPING_EVENT = "pet:finish-typing";

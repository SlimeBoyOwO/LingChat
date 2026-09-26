/**
 * 桌宠显示状态的跨窗口镜像。
 *
 * 宠物窗与气泡窗是两个 webview，Pinia 不共享。气泡窗要画的台词、情绪、标题、
 * 通知都来自宠物窗的 store，所以在宠物窗侧监听这些字段并广播，气泡窗侧应用。
 * 与投屏镜像（`useCastMirror` + `cast:mirror`）同构：显示层单向同步，不反向写回。
 */

/** 气泡窗需要复现的最小显示状态 */
export interface BubbleMirror {
  status: string;
  line: string;
  title: string;
  subtitle: string;
  emotion: string;
  motionText: string;
  /** 打字速度（来自 settings.textSpeed，气泡窗的 uiStore 是只读派生，故直接镜像设置值） */
  textSpeed: number;
  /** 桌宠缩放。气泡窗的窗口尺寸由 Rust 按它创建，CSS 缩放必须用同一个值 ——
   *  否则两边对「窗口有多大」的认知不一致，内容会超出窗口被硬边裁切。 */
  petScale: number;
  notification: { isVisible: boolean; title: string; message: string; type: string };
}

export const PET_BUBBLE_EVENT = "pet:bubble-mirror";
export const PET_BUBBLE_REQUEST = "pet:bubble-request";

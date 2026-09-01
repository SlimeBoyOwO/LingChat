/**
 * auto_send 发送窗口管理（GameDialog / ChatInput 共用，PR 审查统一）：
 * 识别完成 → ASR_AUTO_SEND_DELAY_MS 延迟 → 回调执行发送。
 *
 * - 连续收到 asr-send（窗口内第二次识别）：旧定时器先取消，只保留最新一次。
 *   此前两处组件各自持单个 timer 变量：短时间连续两次识别会覆盖旧引用，
 *   旧 timer 仍在旧时刻触发（组件卸载也只能清掉最后一个）→ 重复发送隐患。
 * - 组件卸载自动取消：窗口未触发就离开页面 → 不发送（消息留在输入框）。
 *
 * 发送时刻的复查（用户编辑尊重 / 清空重填，审查 H1/M3）由组件回调实现——
 * 复查依赖组件自己的输入框 ref，此层只保证"全局唯一窗口"语义。
 */
import { onUnmounted } from "vue";

/** auto_send 模式：识别完成后延迟发送的毫秒数（给用户看到结果的窗口，防乱序）。
 *  导出供 GameDialog / ChatInput 的 asr-send 监听复用（同一延迟语义）。 */
export const ASR_AUTO_SEND_DELAY_MS = 800;

export function useAsrAutoSend(onSend: (detail: string) => void): {
  /** 重置发送窗口：先取消未触发的旧窗口，再从当前时刻起算 */
  arm: (detail: string) => void;
} {
  let timer: number | undefined;

  function arm(detail: string) {
    if (timer !== undefined) window.clearTimeout(timer);
    timer = window.setTimeout(() => {
      timer = undefined;
      onSend(detail);
    }, ASR_AUTO_SEND_DELAY_MS);
  }

  onUnmounted(() => {
    if (timer !== undefined) window.clearTimeout(timer);
    timer = undefined;
  });

  return { arm };
}

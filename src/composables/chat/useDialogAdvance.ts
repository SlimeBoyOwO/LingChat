/**
 * 对话推进状态机 —— 主界面 GameDialog 与桌宠 DialogueBox 共用。
 *
 * 流程：跳过打字动画 → 推进事件队列 → 回调。
 *
 * 两处刻意的差异已按评审结论收敛，因此本文件对桌宠是**行为变更**：
 *   - 桌宠补上「打字中第一次点击先补全文本、不推进」的守卫（此前缺失，
 *     点一下就直接推进，正在打的字被丢掉）
 *   - 桌宠 DialogueBox 里那句游离的 `eventQueue.continue()` 必须同时删除：
 *     它绕过了 continueDialog 的判断，只补守卫不删它会更糟——同一次点击里
 *     文本刚补全、队列就被推进了
 *
 * 动作文本随台词内联显示，只有主界面持有（`inlineMotion`）；桌宠的动作文本由
 * 气泡窗自己持有，不传即完全不介入。
 */
import { useUIStore } from "@/stores/modules/ui/ui";
import { eventQueue } from "@/core/events/event-queue";
import type { ComputedRef } from "vue";

export interface UseDialogAdvanceOptions {
  isTyping: ComputedRef<boolean>;
  /** 补全剩余字符（跳过打字动画） */
  finishTyping: () => void;
  /** 主界面：动作文本随台词内联显示，推进前清掉，等下一句重新写入 */
  inlineMotion?: boolean;
  /** 队列确实向前推进时回调（组件在此 emit player-continued / dialog-proceed） */
  onProceed?: (info: { isPlayerTrigger: boolean; needWait: boolean }) => void;
}

export interface UseDialogAdvanceApi {
  /** 返回 needWait：true 表示队列还需等待（自动推进调度器据此决定要不要排下一轮） */
  continueDialog: (isPlayerTrigger: boolean) => boolean;
}

export function useDialogAdvance(o: UseDialogAdvanceOptions): UseDialogAdvanceApi {
  const uiStore = useUIStore();

  function continueDialog(isPlayerTrigger: boolean): boolean {
    // 打字中：第一次点击跳过动画、显示完整文本（finish 已修复为补全剩余字符）
    if (o.isTyping.value) {
      o.finishTyping();
      return false; // 先跳到末尾，不推进
    }

    // 动作文本已随台词一起显示，推进前清除
    if (o.inlineMotion) {
      uiStore.showCharacterMotionText = "";
    }

    const needWait = eventQueue.continue();
    if (!needWait) {
      o.onProceed?.({ isPlayerTrigger, needWait });
    }

    return needWait;
  }

  return { continueDialog };
}

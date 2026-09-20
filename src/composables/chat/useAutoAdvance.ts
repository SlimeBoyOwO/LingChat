/**
 * 自动推进调度器 —— 主界面 MainChat 与桌宠 PetMode 共用。
 *
 * 事件驱动、非轮询。AUTO 自动模式与台词合并共用一条管道，优先级：
 *   - 台词合并（armed）**严格优先**于 AUTO：armed 时只排合并续打（延迟
 *     mergeLineDelay），AUTO 定时器根本不启动，所以 autoAdvanceDelay 调多小
 *     都不会抢跑 merge。
 *   - 未 armed 且 AUTO 开启：延迟 autoAdvanceDelay 推进下一句。
 *
 * 调度条件（全部满足才推进）：处于 responding、打字机已结束、语音已播完。
 * 触发点：打字结束、音频结束、进入 responding、AUTO 开关变化、合并武装变化。
 *
 * 桌宠通过 `mergeEnabled: false` 关闭合并分支：不读 armed、不维护
 * dialogueMerge.isAudioPlaying，并在 armed 被武装时立即清掉（防止从 /chat
 * 中途离开时残留的 armed 影响桌宠）。其余调度逻辑两模式完全一致。
 */
import { onUnmounted, ref, watch, type Ref } from "vue";
import { dialogueMerge } from "@/core/events/dialogue-merge";
import { eventQueue } from "@/core/events/event-queue";
import { useGameStore } from "@/stores/modules/game";
import { useSettingsStore } from "@/stores/modules/settings";
import { useUIStore } from "@/stores/modules/ui/ui";

export interface AdvanceDialogHandle {
  continueDialog: (isPlayerTrigger: boolean) => boolean;
  readonly isTyping: boolean;
}

export interface UseAutoAdvanceOptions {
  /** 取对话组件句柄的 getter（ref.value） */
  dialog: () => AdvanceDialogHandle | null;
  /** true → 参与台词合并（armed 优先调度并消费）；false → 完全忽略合并 */
  mergeEnabled?: boolean;
}

export interface UseAutoAdvanceApi {
  audioFinished: Ref<boolean>;
  typingFinished: Ref<boolean>;
  /** 绑定到 GameRolesStage 的 @audio-started */
  onAudioStarted: () => void;
  /** 绑定到 GameRolesStage 的 @audio-ended */
  onAudioFinished: () => void;
  /** 绑定到 GameDialog/DialogueBox 的 @player-continued */
  manualTriggerContinue: () => void;
  cancelAdvance: () => void;
  scheduleAdvance: () => void;
  /** AUTO 开关（主界面按钮 / 桌宠按钮共用） */
  toggleAutoMode: () => void;
}

export function useAutoAdvance(o: UseAutoAdvanceOptions): UseAutoAdvanceApi {
  const mergeEnabled = o.mergeEnabled ?? false;

  const uiStore = useUIStore();
  const gameStore = useGameStore();
  const settingsStore = useSettingsStore();

  const typingFinished = ref(true);
  const audioFinished = ref(true);
  let advanceTimer: ReturnType<typeof setTimeout> | null = null;

  const cancelAdvance = () => {
    if (advanceTimer) {
      clearTimeout(advanceTimer);
      advanceTimer = null;
    }
  };

  const scheduleAdvance = () => {
    cancelAdvance();

    if (gameStore.currentStatus !== "responding") return;
    // 实时检查打字机状态（typingFinished 可能还没被打字 watch 同步，微任务顺序不定，
    // 例如 GameDialog 刚消费 armed 开始续打的瞬间，armed watch 先于打字 watch 触发）
    if (o.dialog()?.isTyping || !typingFinished.value || !audioFinished.value) return;

    if (mergeEnabled && dialogueMerge.armed) {
      // 合并续打优先：延迟 mergeLineDelay 后推进队列 → 对话组件对队头短句走追加路径。
      // armed 由对话组件的追加路径消费（保持 true 直到它读到）；队头不是目标则放弃。
      advanceTimer = setTimeout(() => {
        advanceTimer = null;
        // 延迟窗口内可能已被用户手动推进 / 状态变化，重查条件
        if (!dialogueMerge.armed || gameStore.currentStatus !== "responding") return;
        const next = eventQueue.peek();
        if (next?.type === "reply" && next.roleId === dialogueMerge.armedRoleId) {
          o.dialog()?.continueDialog(false);
        } else {
          // 防御：队头不是被合并的那条（被其他事件挡路），放弃合并
          dialogueMerge.armed = false;
        }
      }, settingsStore.text.mergeLineDelay);
    } else if (uiStore.autoMode) {
      advanceTimer = setTimeout(() => {
        advanceTimer = null;
        if (!uiStore.autoMode || gameStore.currentStatus !== "responding") return;
        if (!typingFinished.value || !audioFinished.value) return;

        const needWait = o.dialog()?.continueDialog(false) ?? true;
        if (!needWait) {
          // 推进后重置状态，等待下一条台词的打字/语音事件
          typingFinished.value = true;
          audioFinished.value = true;
        }
      }, settingsStore.autoAdvanceDelay);
    }
  };

  // 音频开始播放：推进挂起，等音频结束
  const onAudioStarted = () => {
    audioFinished.value = false;
    if (mergeEnabled) dialogueMerge.isAudioPlaying = true;
    cancelAdvance();
  };

  // 音频播放结束：可推进（armed 则合并续打，否则 AUTO）
  const onAudioFinished = () => {
    audioFinished.value = true;
    if (mergeEnabled) dialogueMerge.isAudioPlaying = false;
    scheduleAdvance();
  };

  // 用户手动推进：取消当前调度
  const manualTriggerContinue = () => {
    cancelAdvance();
  };

  const toggleAutoMode = () => {
    uiStore.autoMode = !uiStore.autoMode;
  };

  // 监听自动模式开关
  watch(
    () => uiStore.autoMode,
    (enabled) => {
      if (enabled) scheduleAdvance();
      else cancelAdvance();
    },
  );

  // 监听游戏状态：进入 responding 时重置状态并等待事件
  watch(
    () => gameStore.currentStatus,
    (status) => {
      if (status === "responding") {
        typingFinished.value = !(o.dialog()?.isTyping ?? false);
        audioFinished.value = true; // 新台词初始无音频
        scheduleAdvance();
      } else {
        cancelAdvance();
      }
    },
  );

  // 监听打字状态：结束立即尝试推进，开始则取消
  watch(
    () => o.dialog()?.isTyping,
    (typing) => {
      if (typing) {
        typingFinished.value = false;
        cancelAdvance();
      } else {
        typingFinished.value = true;
        scheduleAdvance();
      }
    },
  );

  // 监听合并武装变化：i+1 到达武装 / 被消费时重新调度——armed 时 merge 优先（AUTO 不启动），
  // 武装消费后（追加开始，isTyping 变 true）自动回落 AUTO / 取消。
  watch(
    () => dialogueMerge.armed,
    (armed) => {
      if (mergeEnabled) {
        scheduleAdvance();
      } else if (armed) {
        // 不参与合并：清掉可能从 /chat 残留的武装，避免影响桌宠调度
        dialogueMerge.armed = false;
      }
    },
  );

  // 卸载时清掉音频播放状态与定时器：避免返回后首条回复被当成「续打合并」
  onUnmounted(() => {
    if (mergeEnabled) dialogueMerge.isAudioPlaying = false;
    cancelAdvance();
  });

  return {
    audioFinished,
    typingFinished,
    onAudioStarted,
    onAudioFinished,
    manualTriggerContinue,
    cancelAdvance,
    scheduleAdvance,
    toggleAutoMode,
  };
}

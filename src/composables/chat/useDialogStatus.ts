/**
 * 对话状态 → 占位符状态 + 输入框可用性 —— 主界面 GameDialog 与桌宠 ChatInput 共用。
 *
 * 刻意返回**状态**而不是字符串：两边的占位文案用的词条不同、拼装方式也不同
 * （主界面把「（已深度思考 N 字）」拼在角色提示语后面，桌宠用带插值的词条；
 *  「回应中」桌宠有文案、主界面是空串）。文案属于各模式的呈现层。
 *
 * 共享的是这套易错的状态判定与优先级：录音态优先、思考中取角色提示语、
 * 无角色时回落「等待中」、以及只有 input 态才可输入。
 */
import { computed, watch, type ComputedRef } from "vue";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useAsrInput, asrVoiceActive } from "@/composables/useAsrInput";

export type GameStatus = "input" | "thinking" | "responding" | "presenting";

export type PlaceholderState =
  /** 正在录音：流式 partial 已实时写入输入框，此状态仅兜底非流式 */
  | { kind: "recording" }
  /** 提示语；hint 可能为空串，调用方回退到自己的默认文案 */
  | { kind: "hint"; hint: string }
  | { kind: "thinking"; message: string; length: number }
  /** 思考中但还没有交互角色 */
  | { kind: "waiting" }
  | { kind: "responding" }
  | { kind: "presenting" }
  | { kind: "idle" };

export interface UseDialogStatusOptions {
  /**
   * 状态切换后的模式专属副作用。共享部分只做两件事：
   * 「thinking → 用交互角色的名字/副标题」与「input → 清空情绪」。
   * 其余差异交给调用方，例如 GameDialog 还要清 dialogueMerge.armed、
   * input 时切到用户名、presenting 时清空整行；桌宠没有这些。
   */
  onStatusChange?: (status: GameStatus, prev: GameStatus | undefined) => void;
}

export interface UseDialogStatusApi {
  placeholderState: ComputedRef<PlaceholderState>;
  isInputEnabled: ComputedRef<boolean>;
}

export function useDialogStatus(o?: UseDialogStatusOptions): UseDialogStatusApi {
  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const asrInput = useAsrInput();

  const placeholderState = computed<PlaceholderState>(() => {
    if (asrInput.phase.value === "recording") return { kind: "recording" };

    switch (gameStore.currentStatus) {
      case "input":
        return { kind: "hint", hint: uiStore.showPlayerHintLine };
      case "thinking": {
        const role = gameStore.currentInteractRole;
        if (!role) return { kind: "waiting" };
        return {
          kind: "thinking",
          message: role.thinkMessage,
          length: gameStore.thinkingLength,
        };
      }
      case "responding":
        return { kind: "responding" };
      case "presenting":
        return { kind: "presenting" };
      default:
        return { kind: "idle" };
    }
  });

  // 语音会话进行中禁止手动输入：手打的内容会经 input bridge 污染 ASR 的拼接基准
  const isInputEnabled = computed(
    () => gameStore.currentStatus === "input" && !asrVoiceActive.value,
  );

  watch(
    () => gameStore.currentStatus,
    (status, prev) => {
      if (status === "thinking") {
        const role = gameStore.currentInteractRole;
        if (role) {
          // 思考态不再写入 'AI思考' 伪情感，避免立绘组件因 emotion 残留而无法加载
          uiStore.showCharacterTitle = role.roleName;
          uiStore.showCharacterSubtitle = role.roleSubTitle;
        }
      } else if (status === "input") {
        uiStore.showCharacterEmotion = "";
      }
      o?.onStatusChange?.(status as GameStatus, prev as GameStatus | undefined);
    },
  );

  return { placeholderState, isInputEnabled };
}

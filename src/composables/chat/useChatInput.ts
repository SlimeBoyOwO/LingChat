/** 输入框的文本、发送与 ASR 接线 —— 主界面 GameDialog 与桌宠 ChatInput 共用 */
import { computed, onMounted, onUnmounted, ref, watch, type ComputedRef, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useLlmProvidersStore } from "@/stores/modules/llm-providers";
import { useScreenshot } from "@/composables/useScreenshot";
import { setInputHasText } from "@/composables/useCanDeliver";
import { useRelationGuard } from "@/composables/chat/useRelationGuard";
import {
  ASR_DISPLAY_MS,
  lockAsrForDisplay,
  registerAsrInputBridge,
  useAsrAutoSend,
} from "@/composables/asr";

export interface UseChatInputOptions {
  /** 未选择对话模型时的提示标题词条 key（两模式文案不同） */
  noModelTitleKey?: string;
  /** 未选择对话模型时的提示正文词条 key */
  noModelMessageKey?: string;
}

export interface UseChatInputApi {
  /** 输入框文本。组件可重命名解构以复用原有模板绑定 */
  text: Ref<string>;
  /** IME 组合态（桌宠输入框用于判断是否算"有内容"） */
  isComposing: Ref<boolean>;
  /** 发送按钮禁用依据 */
  isSending: ComputedRef<boolean>;
  /** 是否有草稿（桌宠用它决定失焦后是否保留输入框） */
  hasDraft: () => boolean;
  send: () => void;
}

export function useChatInput(o?: UseChatInputOptions): UseChatInputApi {
  const { t } = useI18n();
  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const llmStore = useLlmProvidersStore();
  const { screenshotBase64, clear: clearScreenshot } = useScreenshot();

  const text = ref("");
  const isComposing = ref(false);

  const isSending = computed(() => gameStore.currentStatus === "thinking");
  const hasDraft = () => text.value.trim() !== "" || isComposing.value;

  // 输入框内容变化 → 通知 can_deliver 追踪（主动搭话的门控之一）
  watch(text, (val) => setInputHasText(Boolean(val.trim())), { immediate: true });

  // AI 回复到达 / 回到 input 态即清空输入框
  watch(
    [() => uiStore.showCharacterLine, () => gameStore.currentStatus],
    ([newLine, newStatus]) => {
      if (newLine && newLine !== "" && newStatus === "responding") {
        text.value = "";
      } else if (newStatus === "input") {
        text.value = "";
      }
    },
  );

  function send() {
    const trimmed = text.value.trim();
    if (!trimmed) return;

    // 检查对话模型是否已选择
    if (!llmStore.chatProviderId) {
      uiStore.showNotification({
        type: "warning",
        title: t(o?.noModelTitleKey ?? "game.dialog.noModelTitle"),
        message: t(o?.noModelMessageKey ?? "game.dialog.noModelMessage"),
        skipTipsCheck: true,
      });
      return;
    }

    gameStore.appendGameMessage({
      type: "message",
      displayName: gameStore.userName,
      content: trimmed,
    });

    // 剧本模式提交给引擎，否则走自由对话
    if (gameStore.runningScript) {
      const script = gameStore.runningScript;
      const wasChoice = script.choices.length > 0;
      invoke("script_submit_input", { input: trimmed })
        .then(() => {
          script.choices = [];
          if (script.freeDialogueInfo.isFreeDialogue) {
            script.freeDialogueInfo.currentRound++;
          }
        })
        .catch((error) => {
          console.error("发送脚本输入失败:", error);
          gameStore.currentStatus = "input";
          uiStore.showNotification({
            type: "warning",
            title: t(wasChoice ? "game.dialog.choiceRequired" : "game.dialog.inputNotAllowed"),
            message: String(error),
            skipTipsCheck: true,
          });
        });
    } else {
      invoke("send_chat_message", {
        text: trimmed,
        screenshotBase64: screenshotBase64.value,
      }).catch((error) => {
        console.error("发送消息失败:", error);
        gameStore.currentStatus = "input";
        // 后端关系就绪门兜底拦下：标记未就绪并弹引导
        if (String(error).includes("RELATION_NOT_READY")) {
          useRelationGuard().block(gameStore.mainRoleId);
        }
      });
    }

    clearScreenshot();
    text.value = "";
  }

  // --- ASR 接线 ---
  function onAsrText(e: Event) {
    const ce = e as CustomEvent<string>;
    if (typeof ce.detail !== "string") return;
    text.value = ce.detail;
    lockAsrForDisplay(ASR_DISPLAY_MS);
  }

  const asrAutoSend = useAsrAutoSend((detail) => {
    if (text.value === detail) send();
  });
  function onAsrAutoSend(e: Event) {
    const ce = e as CustomEvent<string>;
    if (typeof ce.detail !== "string") return;
    text.value = ce.detail;
    asrAutoSend.arm(ce.detail);
  }

  onMounted(() => {
    // 输入桥：流式 partial 实时写入 + 拼接基准读取
    registerAsrInputBridge({
      getText: () => text.value,
      setText: (v) => {
        text.value = v;
      },
    });
    window.addEventListener("asr-text", onAsrText);
    window.addEventListener("asr-send", onAsrAutoSend);
  });

  onUnmounted(() => {
    window.removeEventListener("asr-text", onAsrText);
    window.removeEventListener("asr-send", onAsrAutoSend);
  });

  return { text, isComposing, isSending, hasDraft, send };
}

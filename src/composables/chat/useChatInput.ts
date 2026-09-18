/**
 * 输入框的文本、发送与 ASR 接线 —— 主界面 GameDialog 与桌宠 ChatInput 共用。
 *
 * 收敛的差异（均按评审结论统一）：
 *   - **发送前 trim 首尾空格**（桌宠原本就 trim，主界面没有，但它的守卫
 *     `if (!text.trim())` 说明本意就是要 trim）
 *   - **选项清空改为提交成功后**（桌宠原本在 invoke 之前同步清空，见下方注释）
 *   - 用户消息的追加统一在这里做（桌宠原先提到 PetMode 的 @message-sent 里）
 *
 * 文案与模型检查的提示词条两模式不同，由 noModelTitleKey / noModelMessageKey 传入。
 */
import { computed, onMounted, onUnmounted, ref, watch, type ComputedRef, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useLlmProvidersStore } from "@/stores/modules/llm-providers";
import { useScreenshot } from "@/composables/useScreenshot";
import { setInputHasText } from "@/composables/useCanDeliver";
import {
  ASR_AUTO_SEND_DELAY_MS,
  ASR_DISPLAY_MS,
  lockAsrForDisplay,
  registerAsrInputBridge,
} from "@/composables/useAsrInput";

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
  // （auto_send 识别文本填入后，随回复自动清空）
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
      // 只有提交成功才清空选项。以前是无条件清空的：allow_free 为 false 时后端
      // 会拒绝这次输入，而选项按钮已经消失、引擎仍在等待选择，玩家彻底卡死。
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
      });
    }

    clearScreenshot();
    text.value = "";
  }

  // --- ASR 接线 ---
  // fill_only：识别完成整句填入，由用户手动发送；短暂显示锁防 auto_listen
  // 立即再触发录音、覆盖刚填入的内容（手动触发不受锁限制）
  function onAsrText(e: Event) {
    const ce = e as CustomEvent<string>;
    if (typeof ce.detail !== "string") return;
    text.value = ce.detail;
    lockAsrForDisplay(ASR_DISPLAY_MS);
  }

  // auto_send：先显示到输入框，延迟后走完整 send()（复用剧本分支/模型检查/清理）
  function onAsrAutoSend(e: Event) {
    const ce = e as CustomEvent<string>;
    if (typeof ce.detail !== "string") return;
    text.value = ce.detail;
    window.setTimeout(() => send(), ASR_AUTO_SEND_DELAY_MS);
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

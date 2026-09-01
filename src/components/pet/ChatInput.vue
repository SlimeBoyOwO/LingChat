<template>
  <div
    class="relative z-10 flex w-full justify-center transition-all duration-300 ease-out"
    :class="
      props.visible ? 'translate-y-0 opacity-100' : 'pointer-events-none -translate-y-2 opacity-0'
    "
    :style="{ '--pet-ui-scale': scale }"
  >
    <div
      class="chat-input-container flex items-center rounded-[calc(20px*var(--pet-ui-scale,1))]
        border border-white/10 bg-neutral-950/50 p-[calc(4px*var(--pet-ui-scale,1))] saturate-200
        backdrop-blur-xl"
    >
      <input
        v-model="messageText"
        type="text"
        :placeholder="placeholderText"
        :readonly="!isInputEnabled"
        class="flex-1 border-none bg-transparent p-[calc(5px*var(--pet-ui-scale,1))]
          text-[calc(13px*var(--pet-ui-scale,1))] text-white placeholder-white/40 outline-none
          [text-shadow:0_1px_4px_rgba(0,0,0,0.5)]"
        @keyup.enter="sendMessage"
        @compositionstart="isCompsing = true"
        @compositionend="isCompsing = false"
      />
      <button
        class="relative flex h-6 items-center gap-1 overflow-hidden rounded-full bg-linear-to-tr
          from-cyan-500 to-blue-400 px-2 text-sm font-bold text-white
          shadow-[0_4px_15px_rgba(6,182,212,0.4)] transition-all duration-300 hover:from-cyan-400
          hover:to-blue-300 hover:shadow-[0_6px_20px_rgba(6,182,212,0.6)] active:scale-95"
        @click="sendMessage"
        :disabled="!isInputEnabled"
      >
        <div
          class="pointer-events-none absolute top-0 left-0 h-1/2 w-full rounded-t-full bg-white/20"
        ></div>
        <Forward />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { ref, watch, computed, onMounted, onUnmounted } from "vue";
  import { useI18n } from "vue-i18n";
  import { invoke } from "@tauri-apps/api/core";
  import { useGameStore } from "@/stores/modules/game";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { useSettingsStore } from "@/stores/modules/settings";
  import { useLlmProvidersStore } from "@/stores/modules/llm-providers";
  import {
    useAsrInput,
    registerAsrInputBridge,
    lockAsrForDisplay,
    asrVoiceActive,
  } from "@/composables/useAsrInput";
  import { useAsrAutoSend } from "@/composables/useAsrAutoSend";
  import { useScreenshot } from "@/composables/useScreenshot";
  import { setInputHasText } from "@/composables/useCanDeliver";
  import { Forward } from "lucide-vue-next";

  const { t } = useI18n();
  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const settingsStore = useSettingsStore();
  const llmStore = useLlmProvidersStore();

  const {
    screenshotBase64,
    init: initScreenshot,
    destroy: destroyScreenshot,
    clear: clearScreenshot,
  } = useScreenshot();

  // fill_only：识别完成整句填入（与桌面 onAsrText 一致）；400ms 显示锁防
  // auto RMS 识别完立即再触发覆盖刚填入的内容（手动不受锁限）
  const ASR_DISPLAY_MS = 400;
  function onAsrText(e: Event) {
    const ce = e as CustomEvent<string>;
    if (typeof ce.detail === "string") {
      messageText.value = ce.detail;
      lockAsrForDisplay(ASR_DISPLAY_MS);
    }
  }

  // auto_send：识别结果显示到输入框 → ASR_AUTO_SEND_DELAY_MS 后走 sendMessage()
  //（完整复用剧本分支/模型检查/输入框清理；显示锁已由 handle() 设置）。
  // 发送后 emit asr-auto-sent：PetMode 据此收起输入框（内容已交给 LLM，无需
  // 再展示；fill_only 不发该事件——识别结果要留在输入框等用户手动发送）
  // auto_send 发送窗口：useAsrAutoSend 统一管理（连续识别先清旧 timer、卸载
  // 自动取消，防离开桌宠后 timer 仍触发发送——审查统一）
  const asrAutoSend = useAsrAutoSend((detail) => {
    // 发送时刻复查（审查 H1/M3）：用户编辑了输入框（非空且不等于识别结果）→
    // 尊重编辑不发送；被清空（AI 回复开始等）→ 重填识别结果再发，语音内容不丢。
    // 内容比对同时天然防重复：800ms 窗口内第二次识别时旧 timer 已被清除，
    // 只有最新一次真正发送
    if (messageText.value === "") messageText.value = detail;
    if (messageText.value === detail) {
      sendMessage();
      emit("asr-auto-sent");
    }
  });
  function onAsrAutoSend(e: Event) {
    const ce = e as CustomEvent<string>;
    if (typeof ce.detail !== "string") return;
    messageText.value = ce.detail;
    asrAutoSend.arm(ce.detail);
  }

  // 输入桥：流式 partial 实时写入（与桌面 GameDialog 一致；录音发起窗口的
  // phase 是窗口本地状态，partial 只写入发起方输入框）
  const asrInput = useAsrInput();
  onMounted(() => {
    initScreenshot();
    registerAsrInputBridge({
      getText: () => messageText.value,
      setText: (v) => {
        messageText.value = v;
      },
    });
    window.addEventListener("asr-text", onAsrText);
    window.addEventListener("asr-send", onAsrAutoSend);
  });
  onUnmounted(() => {
    window.removeEventListener("asr-text", onAsrText);
    window.removeEventListener("asr-send", onAsrAutoSend);
    // auto_send 发送窗口未触发就离开 → useAsrAutoSend 卸载自动取消（消息留输入框）
    destroyScreenshot();
  });

  // 与主对话窗口行为一致（GameDialog 同款 watch）：AI 回复（showCharacterLine
  // 非空 + responding）时清空输入框——auto_send 识别文本填入后随回复自动清空
  watch(
    [() => uiStore.showCharacterLine, () => gameStore.currentStatus],
    ([newLine, newStatus]) => {
      if (newLine && newLine !== "" && newStatus === "responding") {
        messageText.value = "";
      }
    }
  );

  const scale = computed(() => settingsStore.pet?.scale || 1.0);

  const placeholderText = computed(() => {
    switch (gameStore.currentStatus) {
      case "input":
        return uiStore.showPlayerHintLine || t("views.pet.chatInput.placeholder");
      case "thinking":
        const currentInteractRole = gameStore.currentInteractRole;
        if (currentInteractRole) {
          const baseMessage = currentInteractRole.thinkMessage;
          if (gameStore.thinkingLength > 0) {
            return t("views.pet.chatInput.deepThought", {
              message: baseMessage,
              length: gameStore.thinkingLength,
            });
          }
          return baseMessage;
        } else {
          return t("views.pet.chatInput.waiting");
        }
      case "responding":
        return t("views.pet.chatInput.chatting");
      case "presenting":
        return "";
      default:
        return t("views.pet.chatInput.placeholderDefault");
    }
  });

  watch(
    () => gameStore.currentStatus,
    (newStatus) => {
      console.log("游戏状态变为 :", newStatus);
      if (newStatus === "thinking") {
        const currentInteractRole = gameStore.currentInteractRole;
        if (currentInteractRole) {
          // 思考态不再写入 'AI思考' 伪情感，避免立绘组件因 emotion 残留而无法加载
          uiStore.showCharacterTitle = currentInteractRole.roleName;
          uiStore.showCharacterSubtitle = currentInteractRole.roleSubTitle;
        }
      } else if (newStatus === "input") {
        uiStore.showCharacterEmotion = "";
      }
    }
  );

  // 录音期间输入框只读（与桌面 GameDialog 一致）：击键声会混入麦克风采样送识别
  const isInputEnabled = computed(
    () => gameStore.currentStatus === "input" && !asrVoiceActive.value
  );

  const props = defineProps({
    visible: {
      type: Boolean,
      default: false,
    },
  });

  const emit = defineEmits(["message-sent", "asr-auto-sent"]);

  const messageText = ref("");
  // 输入框内容变化 → 通知 can_deliver 追踪
  watch(messageText, (val) => setInputHasText(Boolean(val.trim())), { immediate: true });

  const isCompsing = ref(false);
  const isTyping = () => messageText.value.trim() != "" || isCompsing.value;
  defineExpose({ isTyping });

  const sendMessage = () => {
    const text = messageText.value.trim();
    if (!text) return;

    // 检查对话模型是否已选择
    if (!llmStore.chatProviderId) {
      uiStore.showNotification({
        type: "warning",
        title: t("views.pet.chatInput.noModelTitle"),
        message: t("views.pet.chatInput.noModelMessage"),
        skipTipsCheck: true,
      });
      return;
    }

    if (gameStore.runningScript) {
      invoke("script_submit_input", { input: text }).catch((error) => {
        console.error("发送脚本输入失败:", error);
        gameStore.currentStatus = "input";
      });
      gameStore.runningScript.choices = [];
      if (gameStore.runningScript.freeDialogueInfo.isFreeDialogue) {
        gameStore.runningScript.freeDialogueInfo.currentRound++;
      }
    } else {
      invoke("send_chat_message", { text, screenshotBase64: screenshotBase64.value }).catch(
        (error) => {
          console.error("发送消息失败:", error);
          gameStore.currentStatus = "input";
        }
      );
    }

    emit("message-sent", text);
    messageText.value = "";
    clearScreenshot();
  };
</script>

<style scoped></style>

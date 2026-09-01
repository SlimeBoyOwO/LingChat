<template>
  <div
    id="pet-app"
    :style="appStyleVars"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    class="relative flex h-(--app-height) w-(--app-width) flex-col items-center justify-start
      overflow-hidden bg-transparent transition-none select-none"
  >
    <!-- DialogueBox 区域 -->
    <div
      class="flex w-full shrink-0 flex-col justify-end bg-transparent transition-none"
      :style="{ height: 'var(--dialog-h)' }"
    >
      <PetNotification />
      <div class="mt-1 flex items-end justify-center">
        <DialogueBox
          ref="gameDialogRef"
          @player-continued="manualTriggerContinue"
          @dialog-proceed="resetInteraction"
        />
      </div>
    </div>

    <!-- Avatar 区域 -->
    <DragArea :isDragging="isDragging">
      <div
        ref="avatarContainer"
        class="flex shrink-0 items-center justify-center bg-transparent transition-all duration-100"
        :style="{ width: 'var(--avatar-size)', height: 'var(--avatar-size)' }"
      >
        <GameRolesStage
          @avatar-click="handleAvatarClick"
          @open-settings="handleOpenSettings"
          @switch-auto-mode="handleSwitchAutoMode"
          @exit-pet-mode="handleExitPetMode"
          @audio-ended="handleAudioFinished"
          @audio-started="handleAudioStarted"
        />
      </div>
    </DragArea>

    <!-- ChatInput 区域 -->
    <div
      ref="chatContainer"
      class="flex w-full shrink-0 items-start justify-center bg-transparent transition-none"
      :style="{ height: 'var(--chat-h)' }"
    >
      <ChatInput
        ref="ChatInputRef"
        :visible="showChatInput"
        @message-sent="handleMessageSent"
        @asr-auto-sent="handleAsrAutoSent"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
  import { eventQueue } from "@/core/events/event-queue";
  import { useGameStore } from "@/stores/modules/game";
  import { useSettingsStore } from "@/stores/modules/settings";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { invoke } from "@tauri-apps/api/core";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { computed, onMounted, onUnmounted, ref, watch } from "vue";
  import { useI18n } from "vue-i18n";
  import { useRouter } from "vue-router";
  import { useFileDrop } from "../pet/useFileDrop";
  import { useAsrInput } from "@/composables/useAsrInput";

  import ChatInput from "../pet/ChatInput.vue";
  import DialogueBox from "../pet/DialogueBox.vue";
  import DragArea from "../pet/DragArea.vue";
  import GameRolesStage from "../pet/GameRolesStage.vue";
  import PetNotification from "../pet/PetNotification.vue";
  import { BASE_AVATAR_SIZE, CHAT_BASE_H, DIALOG_MAX_BASE } from "../pet/constants";

  const { t } = useI18n();
  const router = useRouter();
  const gameStore = useGameStore();
  const settingsStore = useSettingsStore();
  const uiStore = useUIStore();

  const showChatInput = ref(false);
  const { isDragging, hasFile } = useFileDrop();

  const avatarContainer = ref<HTMLElement | null>(null);
  const chatContainer = ref<HTMLElement | null>(null);
  const gameDialogRef = ref<InstanceType<typeof DialogueBox> | null>(null);
  const ChatInputRef = ref<InstanceType<typeof ChatInput> | null>(null);

  const appStyleVars = computed(() => {
    const scale = settingsStore.pet?.scale || 1.0;
    const layout = calcWindowLayout(scale);
    return {
      "--pet-ui-scale": scale.toString(),
      "--app-width": `${layout.width}px`,
      "--app-height": `${layout.height}px`,
      "--avatar-size": `${Math.round(BASE_AVATAR_SIZE * scale)}px`,
      "--chat-h": `${Math.round(CHAT_BASE_H * scale)}px`,
      "--dialog-h": `${Math.round(DIALOG_MAX_BASE * scale)}px`,
    };
  });

  const calcWindowLayout = (scale: number): { width: number; height: number } => {
    const S = Math.round(BASE_AVATAR_SIZE * scale);
    const chatH = Math.round(CHAT_BASE_H * scale);
    const dialogH = Math.round(DIALOG_MAX_BASE * scale);
    return { width: S, height: S + dialogH + chatH };
  };

  const applyWindowLayout = async () => {
    try {
      const scale = settingsStore.pet?.scale || 1.0;
      await invoke("set_pet_mode", { enable: true, scale });
    } catch (error) {
      console.error("调整窗口布局失败:", error);
    }
  };

  let hitTestInterval: number | undefined;
  let scaleUnlisten: (() => void) | null = null;
  let effectUnlisten: (() => void) | null = null;
  let volumeUnlisten: (() => void) | null = null;
  let dialogHistoryUnlisten: (() => void) | null = null;

  onMounted(async () => {
    const appWindow = getCurrentWindow();

    // 识别结果填入输入框（fill_only 的 asr-text / auto_send 的 asr-send）→ 展开：
    // 录音中鼠标可能已移出（输入框已收起），此时才展开由 isTyping 保持显示；
    // auto_send 路径 800ms 后由 asr-auto-sent 收起，展示窗口完整
    window.addEventListener("asr-text", handleAsrFilled);
    window.addEventListener("asr-send", handleAsrFilled);

    scaleUnlisten = await appWindow.listen<{ scale: number }>("pet-scale-changed", (event) => {
      const scale = Number(event.payload?.scale);
      if (!Number.isNaN(scale)) {
        settingsStore.pet.scale = scale;
        void applyWindowLayout();
      }
    });

    effectUnlisten = await appWindow.listen<{ effect: string }>(
      "background-effect-changed",
      (event) => {
        const effect = event.payload?.effect;
        if (effect) {
          uiStore.setBackgroundEffect(effect);
        }
      }
    );

    volumeUnlisten = await appWindow.listen<{ volume: number }>("pet-volume-changed", (event) => {
      const volume = Number(event.payload?.volume);
      if (!Number.isNaN(volume)) {
        settingsStore.updateAudio({ characterVolume: volume });
      }
    });

    // 响应设置窗口的初始历史数据请求
    dialogHistoryUnlisten = await appWindow.listen("request-dialog-history", () => {
      appWindow.emit("dialog-history-changed", {
        dialogHistory: JSON.parse(JSON.stringify(gameStore.dialogHistory)),
      });
    });

    // 设置透明背景的 body 属性样式（额外防护）
    document.body.style.backgroundColor = "transparent";
    document.documentElement.style.backgroundColor = "transparent";

    // 1. 初始化窗口为桌宠尺寸
    await applyWindowLayout();

    // 2. 启动 100ms 一次的 solid bounds 测试
    hitTestInterval = window.setInterval(() => {
      const rects = [];

      // 如果对话气泡正在显示，则加入 solid region（用气泡元素精确 rect，避免包住整个对话框）
      if (
        gameDialogRef.value?.bubbleRef &&
        gameStore.currentStatus === "responding" &&
        gameStore.currentLine.trim() !== ""
      ) {
        const r = gameDialogRef.value.bubbleRef.getBoundingClientRect();
        if (r.height > 0) {
          rects.push({ x: r.x, y: r.y, width: r.width, height: r.height });
        }
      }

      // 头像圆环常驻 solid region 触发拖拽和交互
      if (avatarContainer.value) {
        const r = avatarContainer.value.getBoundingClientRect();
        rects.push({ x: r.x, y: r.y, width: r.width, height: r.height });
      }

      // 输入框显示时，加入 solid region
      if (chatContainer.value && showChatInput.value) {
        const r = chatContainer.value.getBoundingClientRect();
        // 输入框稍微拓宽，保证极小尺寸下的鼠标判定连贯性
        rects.push({
          x: r.x - 20,
          y: r.y - 20,
          width: r.width + 40,
          height: r.height + 40,
        });
      }

      invoke("update_solid_regions", { rects }).catch(console.error);
    }, 100);
  });

  watch(
    () => settingsStore.pet?.scale,
    () => {
      void applyWindowLayout();
    }
  );

  // 监听 dialogHistory 变化，推送给设置窗口
  watch(
    () => gameStore.dialogHistory.length,
    () => {
      const appWindow = getCurrentWindow();
      appWindow.emit("dialog-history-changed", {
        dialogHistory: JSON.parse(JSON.stringify(gameStore.dialogHistory)),
      });
    }
  );

  onUnmounted(() => {
    // 恢复默认背景色
    document.body.style.backgroundColor = "";
    document.documentElement.style.backgroundColor = "";

    if (scaleUnlisten) scaleUnlisten();
    if (effectUnlisten) effectUnlisten();
    if (volumeUnlisten) volumeUnlisten();
    if (dialogHistoryUnlisten) dialogHistoryUnlisten();

    if (hitTestInterval !== undefined) {
      window.clearInterval(hitTestInterval);
    }
    window.removeEventListener("asr-text", handleAsrFilled);
    window.removeEventListener("asr-send", handleAsrFilled);
  });

  const handleMessageSent = (message: string) => {
    gameStore.appendGameMessage({
      type: "message",
      displayName: gameStore.userName,
      content: message,
    });
  };

  const asrInput = useAsrInput();
  // 语音快捷键（PTT）/mic 触发录音 → 展开输入框：桌宠鼠标离开即收起输入框
  // （handleMouseLeave），用户用全局快捷键在别的应用前台说话时看不到识别
  // 结果——录音开始即展开，fill_only 填入后由 isTyping 保持显示。
  // auto 源不展开：自动监听常开，不应每次说话都弹出输入框（幂等：mic 按钮
  // 点击时鼠标必在桌宠上，输入框本就已展开）
  watch([asrInput.phase, asrInput.activeSource], ([p, s]) => {
    if (p === "recording" && s === "button") showChatInput.value = true;
  });
  // auto_send 语音发送完成（ChatInput 已清空输入框）→ 收起输入框：
  // 内容已交给 LLM 无需再展示；fill_only 不发此事件，识别结果留在输入框
  const handleAsrAutoSent = () => {
    showChatInput.value = false;
  };
  // 识别结果填入（onMounted 注册 asr-text/asr-send 监听）：鼠标移出后输入框
  // 已收起时重新展开，让用户看到识别内容
  const handleAsrFilled = () => {
    showChatInput.value = true;
  };

  const handleMouseEnter = () => {
    showChatInput.value = true;
  };

  const handleMouseLeave = () => {
    if (ChatInputRef.value?.isTyping()) {
      showChatInput.value = true;
      return;
    } else {
      showChatInput.value = false;
    }
  };

  const handleAvatarClick = () => {
    manualTriggerContinue();
    eventQueue.continue();
    resetInteraction();
  };

  const handleOpenSettings = async () => {
    try {
      const existing = await WebviewWindow.getByLabel("settings");
      if (existing) {
        await existing.setFocus();
        return;
      }

      const webview = new WebviewWindow("settings", {
        url: "/second",
        title: t("views.petMode.settingsWindowTitle"),
        width: 1200,
        height: 800,
        resizable: true,
        shadow: false,
        decorations: false,
        transparent: true,
        alwaysOnTop: false,
      });

      webview.once("tauri://created", () => {
        console.log("桌宠轻量设置窗口创建成功");
      });

      webview.once("tauri://error", (e) => {
        console.error("创建桌宠轻量设置窗口失败:", e);
      });
    } catch (error) {
      console.error("打开设置窗口时出错:", error);
    }
  };

  // 自动打字/对话逻辑
  let timerId: any = null;
  const isContinueTriggered = ref(false);
  const audioFinished = ref(true);

  const resetInteraction = () => {
    isContinueTriggered.value = false;
    audioFinished.value = true;
    if (timerId) {
      clearTimeout(timerId);
      timerId = null;
    }
  };

  const tryAutoAdvance = () => {
    if (!uiStore.autoMode) return;
    if (isContinueTriggered.value) return;
    if (gameStore.currentStatus !== "responding") return;

    const typing = gameDialogRef.value?.isTyping ?? false;
    if (typing || !audioFinished.value) return;

    if (timerId) clearTimeout(timerId);
    timerId = setTimeout(() => {
      if (gameDialogRef.value) {
        const needWait = gameDialogRef.value.continueDialog(false);
        if (needWait) {
          tryAutoAdvance();
        }
      }
    }, settingsStore.autoAdvanceDelay);
  };

  const handleAudioStarted = () => {
    audioFinished.value = false;
  };

  const handleAudioFinished = () => {
    audioFinished.value = true;
    tryAutoAdvance();
  };

  watch(
    () => gameDialogRef.value?.isTyping,
    (typing) => {
      if (typing === false) {
        tryAutoAdvance();
      }
    }
  );

  const manualTriggerContinue = () => {
    if (timerId) {
      clearTimeout(timerId);
      timerId = null;
    }
    if (!isContinueTriggered.value) {
      isContinueTriggered.value = true;
    }
  };

  const handleSwitchAutoMode = () => {
    uiStore.autoMode = !uiStore.autoMode;
  };

  const handleExitPetMode = async () => {
    // 关闭设置窗口（如果打开的话）
    try {
      const settingsWindow = await WebviewWindow.getByLabel("settings");
      if (settingsWindow) {
        await settingsWindow.close();
      }
    } catch {
      // 窗口不存在，忽略
    }

    // 退出时清除 solid region，防止残留
    await invoke("update_solid_regions", { rects: [] });
    // 1. 关闭桌宠窗口特性，恢复 1500x800 的正常主窗口
    await invoke("set_pet_mode", { enable: false });
    // 2. 路由导航回聊天主页面
    router.push("/chat");
  };
</script>

<style scoped>
  #pet-app {
    position: relative;
    width: 100vw;
    height: 100dvh;
    overflow: hidden;
  }
</style>

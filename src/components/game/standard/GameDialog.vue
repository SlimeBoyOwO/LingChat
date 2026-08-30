<template>
  <div
    class="game-dialog relative z-2 flex w-full scrollbar-thin [scrollbar-color:var(--accent-color)_transparent]
      justify-center p-3.75 transition-all duration-200 ease-[cubic-bezier(0.25,0.46,0.45,0.94)]
      before:pointer-events-none before:absolute before:-top-10 before:right-0 before:left-0
      before:h-10 before:bg-linear-to-b before:from-transparent before:via-[rgba(0,14,39,0.3)]
      before:to-[rgba(0,14,39,0.6)] before:content-['']"
    :class="{
      [`z-[-1]! overflow-hidden opacity-0 duration-500! ease-linear before:opacity-0
      before:duration-1000!`]: isHidden,
      'max-h-[40dvh]': !uiStore.isNarrowScreen,
    }"
    :style="dialogWrapperStyle"
    @wheel="handleWheelHistory"
  >
    <div :style="{ width: containerWidth + '%' }" class="relative">
      <div class="overflow-y-auto">
        <!-- 标题栏 -->
        <div class="mb-2 flex items-baseline">
          <!-- 角色名称 + 副标题：两者一起切换，旧标题向上滑出、新标题从上方滑入 -->
          <Transition name="title-slide">
            <div
              :key="titleSubtitleKey"
              class="flex items-baseline"
              :class="{ 'min-w-0': uiStore.isNarrowScreen }"
            >
              <div
                class="mr-3.75 font-[inherit] text-2xl font-bold text-shadow-[inherit]"
                :class="{
                  'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap': uiStore.isNarrowScreen,
                }"
                :style="{ color: dialogTextColorValue }"
              >
                <div id="character">{{ uiStore.showCharacterTitle }}</div>
              </div>
              <div
                v-show="!uiStore.isNarrowScreen"
                class="font-[inherit] text-xl font-bold text-[#6eb4ff] text-shadow-[inherit]"
              >
                <div id="character-sub">{{ uiStore.showCharacterSubtitle }}</div>
              </div>
            </div>
          </Transition>

          <!-- 情绪标签 -->
          <div
            class="relative mx-4 shrink-0 font-[inherit] text-xl font-bold whitespace-nowrap
              text-[#ff77dd] text-shadow-[inherit]"
          >
            <Transition name="emotion-slide">
              <div id="character-emotion" :key="uiStore.showCharacterEmotion" class="inline-block">
                {{ uiStore.showCharacterEmotion }}
              </div>
            </Transition>
          </div>

          <!-- 操作按钮组配置 -->
          <div class="ml-auto flex min-w-0 items-baseline">
            <!-- 桌面端：直接显示所有操作按钮 -->
            <template v-if="!isMobile">
              <!-- 操作按钮组 -->
              <div
                class="custom-scroll overflow-x-auto"
                :class="uiStore.isNarrowScreen ? 'min-w-0 flex-1' : 'shrink-0'"
              >
                <div class="flex whitespace-nowrap">
                  <Button
                    type="nav"
                    icon="background"
                    :title="$t('game.dialog.sceneSettings')"
                    @click="openSceneSettings"
                  ></Button>
                  <!--
                  <Button
                    type="nav"
                    icon="hand"
                    :title="$t('game.dialog.touchMode')"
                    @click="toggleTouchMode"
                    @contextmenu.prevent="exitTouchMode"
                  ></Button>
                  -->
                  <Button
                    type="nav"
                    icon="history"
                    :title="$t('game.dialog.history')"
                    @click="openHistory"
                  ></Button>

                  <!-- 语音输入按钮（auto_listen 开启时变为关闭开关） -->
                  <Button
                    type="nav"
                    :icon="micIcon"
                    :title="micTitle"
                    :class="{
                      [`animate-asr-breathe
                      text-blue-500`]: asrInput.phase.value === 'recording',
                    }"
                    :disabled="!canStartMic"
                    @click="toggleRecording"
                  ></Button>

                  <div class="group relative inline-flex">
                    <div
                      v-if="hasScreenshot"
                      class="pointer-events-none absolute bottom-full left-1/2 z-50 mb-2
                        -translate-x-1/2 opacity-0 transition-opacity duration-200
                        group-hover:opacity-100"
                    >
                      <img
                        :src="'data:image/jpeg;base64,' + screenshotBase64"
                        class="max-h-64 max-w-96 rounded-lg border-2 object-contain shadow-lg"
                        style="border-color: var(--accent-color); background: #000"
                      />
                    </div>
                    <Button
                      type="nav"
                      icon="camera"
                      :title="
                        hasScreenshot
                          ? $t('game.dialog.screenshotRetake')
                          : $t('game.dialog.screenshotAsk')
                      "
                      :style="hasScreenshot ? { color: 'var(--accent-color)' } : {}"
                      @click="startScreenshot"
                      @contextmenu.prevent="clearScreenshot"
                    ></Button>
                  </div>

                  <Button
                    type="nav"
                    icon="close"
                    :title="$t('game.dialog.closeDialog')"
                    @click="removeDialog"
                  ></Button>
                </div>
              </div>
            </template>

            <!-- 移动端：箭头折叠按钮 + 关闭按钮 -->
            <div v-if="isMobile" class="flex items-baseline gap-1">
              <button
                class="mobile-toggle-btn"
                :class="{ 'is-open': showMobileMenu }"
                :title="$t('game.dialog.moreActions')"
                @click="showMobileMenu = !showMobileMenu"
              >
                ▲
              </button>
              <Button
                type="nav"
                icon="close"
                :title="$t('game.dialog.closeDialog')"
                @click="removeDialog"
              ></Button>
            </div>
          </div>
        </div>

        <!-- 移动端：折叠菜单下拉面板 -->
        <Transition name="mobile-menu">
          <div v-if="isMobile && showMobileMenu" class="mobile-menu-dropdown">
            <div class="custom-scroll flex gap-1 overflow-x-auto pb-1 whitespace-nowrap">
              <Button
                type="nav"
                icon="background"
                :title="$t('game.dialog.sceneSettings')"
                @click="onMobileMenuAction(openSceneSettings)"
              ></Button>
              <Button
                type="nav"
                icon="hand"
                :title="$t('game.dialog.touchMode')"
                @click="onMobileMenuAction(toggleTouchMode)"
                @contextmenu.prevent="exitTouchMode"
              ></Button>
              <Button
                type="nav"
                icon="history"
                :title="$t('game.dialog.history')"
                @click="onMobileMenuAction(openHistory)"
              ></Button>
              <Button
                type="nav"
                :icon="micIcon"
                :title="micTitle"
                :class="{
                  [`animate-asr-breathe
                  text-blue-500`]: asrInput.phase.value === 'recording',
                }"
                :disabled="!canStartMic"
                @click="onMobileMenuAction(toggleRecording)"
              ></Button>
              <div class="group relative inline-flex">
                <div
                  v-if="hasScreenshot"
                  class="pointer-events-none absolute bottom-full left-1/2 z-50 mb-2
                    -translate-x-1/2 opacity-0 transition-opacity duration-200
                    group-hover:opacity-100"
                >
                  <img
                    :src="'data:image/jpeg;base64,' + screenshotBase64"
                    class="max-h-64 max-w-96 rounded-lg border-2 object-contain shadow-lg"
                    style="border-color: var(--accent-color); background: #000"
                  />
                </div>
                <Button
                  type="nav"
                  icon="camera"
                  :title="
                    hasScreenshot
                      ? $t('game.dialog.screenshotRetake')
                      : $t('game.dialog.screenshotAsk')
                  "
                  :style="hasScreenshot ? { color: 'var(--accent-color)' } : {}"
                  @click="onMobileMenuAction(startScreenshot)"
                  @contextmenu.prevent="onMobileMenuAction(clearScreenshot)"
                ></Button>
              </div>
            </div>
          </div>
        </Transition>

        <!-- 分割线：青蓝色发光线条，亮段从左向右流动（同源桌宠外框 sweep-glow-ring） -->
        <div class="dialog-divider-glow my-1.5"></div>

        <!-- 输入区 -->
        <div
          class="my-1.25 flex min-h-10 w-full resize-none flex-col border-none bg-transparent
            text-xl font-bold whitespace-pre-line text-white transition-all duration-300
            outline-none"
        >
          <!-- AI 回复显示区（仅回应状态可见；标准/内联模式共用，逐字符淡入+上浮） -->
          <div
            v-show="currentStatus === 'responding'"
            ref="inlineDisplayRef"
            tabindex="0"
            class="response-display my-1.25 max-h-[50dvh] min-h-30 flex-1 resize-none overflow-y-auto
              border-none bg-transparent font-[inherit] text-xl font-bold break-all
              whitespace-pre-line outline-none text-shadow-[inherit]"
            :class="textareaMotionClass"
            @keydown.enter.exact.prevent="sendOrContinue"
          ></div>

          <!-- 输入框 textarea（非回应状态：思考/输入/展示阶段） -->
          <textarea
            v-show="currentStatus !== 'responding'"
            id="inputMessage"
            ref="textareaRef"
            class="my-1.25 max-h-[50dvh] min-h-30 flex-1 resize-none border-none bg-transparent
              font-[inherit] text-[max(1.25rem,16px)] font-bold transition-all duration-300 outline-none
              text-shadow-[inherit] placeholder:text-white/50 placeholder:shadow-none"
            :placeholder="placeholderText"
            v-model="inputMessage"
            @keydown.enter.exact.prevent="sendOrContinue"
            :readonly="!isInputEnabled"
          ></textarea>
        </div>
      </div>
      <!-- 发送按钮（内层右侧外部） -->
      <button
        id="sendButton"
        class="absolute right-0 bottom-0 translate-x-full cursor-pointer rounded-[5px] border-none
          bg-transparent px-2 py-2 font-[inherit] text-sm font-bold text-[#04bcff] transition-all
          duration-300 text-shadow-[inherit] hover:bg-transparent
          hover:text-[rgba(136,255,251,0.827)] disabled:cursor-not-allowed disabled:bg-[#333]
          disabled:opacity-70"
        :disabled="isSending"
        @click="sendOrContinue"
      >
        ▼
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { computed, onMounted, onUnmounted, ref, watch } from "vue";
  import { useI18n } from "vue-i18n";
  import { useTypeWriter } from "../../../composables/ui/useTypeWriter";
  import { setInputHasText } from "../../../composables/useCanDeliver";
  import { useDialogAppearance } from "../../../composables/useDialogAppearance";
  import { eventQueue } from "../../../core/events/event-queue";
  import { useGameStore } from "../../../stores/modules/game";
  import { useLlmProvidersStore } from "../../../stores/modules/llm-providers";
  import { useSettingsStore } from "../../../stores/modules/settings";
  import { useDialogStore } from "../../../stores/modules/ui/dialog";
  import { useUIStore } from "../../../stores/modules/ui/ui";
  import { escapeHtml } from "../../../utils/escapeHtml";
  import { createCharRevealWriter } from "../../../utils/typewriter/charReveal";
  import {
    useAsrInput,
    setMobileMenuOpen,
    lockAsrForDisplay,
    registerAsrInputBridge,
    asrVoiceActive,
    ASR_AUTO_SEND_DELAY_MS,
  } from "../../../composables/useAsrInput";
  import { useAsrStore } from "../../../stores/modules/settings/asr";
  import { Button } from "../../base";

  const inputMessage = ref("");
  const { t } = useI18n();
  // 输入框内容变化 → 通知 can_deliver 追踪
  watch(inputMessage, (val) => setInputHasText(Boolean(val.trim())), { immediate: true });
  const isShowingMotionText = ref(false);
  const textareaRef = ref<HTMLTextAreaElement | null>(null);
  const inlineDisplayRef = ref<HTMLDivElement | null>(null);
  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const dialogStore = useDialogStore();
  const settingsStore = useSettingsStore();
  const llmStore = useLlmProvidersStore();

  // Dialog appearance managed by composable: useDialogAppearance
  const { isHidden, hide, dialogWrapperStyle, dialogTextColorValue, handleWheelHistory } =
    useDialogAppearance({
      openHistory: () => {
        uiStore.toggleSettings(true);
        uiStore.setSettingsTab("history");
      },
    });

  // 移动端按钮折叠状态（但是基于长宽比判断）
  const isMobile = ref(uiStore.aspectRatio <= 1);
  const showMobileMenu = ref(false);
  // 同步给 ASR 模块：移动端菜单展开时禁用语音输入（§1.5）
  watch(showMobileMenu, (open) => setMobileMenuOpen(open));

  // 当前游戏状态（模板 v-show 判定回复显示区 / 输入框）
  const currentStatus = computed(() => gameStore.currentStatus);

  // 标题栏（角色名 + 副标题）切换 key：任一变化时整体一起滑出/滑入
  const titleSubtitleKey = computed(
    () => `${uiStore.showCharacterTitle}|${uiStore.showCharacterSubtitle}`,
  );

  // 语音输入：useAsrInput 统一两种触发源（mic 按钮 / 自动监听），
  // 替换上游的 Web Speech API 实现（状态为模块级单例，GameRolesStage 等共享）
  const asrInput = useAsrInput();
  const asrStore = useAsrStore();

  // auto_listen 模式开 + 总开关开：mic 按钮 = 功能开关（监听激活 → 暂停；暂停 → 恢复），
  // 不改模式设置。总开关关（自动模式已停）→ 退化为手动录音。
  const autoListenOn = computed(() => asrStore.settings.auto_listen);
  const autoListenActive = computed(() => asrInput.autoListenActive.value);
  const micIcon = computed(() => {
    if (autoListenOn.value && asrStore.settings.voice_input_enabled) {
      return autoListenActive.value ? "mic-off" : "mic";
    }
    return "mic";
  });
  const micTitle = computed(() => {
    if (autoListenOn.value && asrStore.settings.voice_input_enabled) {
      return autoListenActive.value
        ? t("game.dialog.asrAutoOff") // 监听中：暂停
        : t("game.dialog.asrAutoResume"); // 已暂停：恢复
    }
    return asrInput.phase.value === "recording"
      ? t("game.dialog.recordingStop")
      : t("game.dialog.voiceInput");
  });

  // mic 按钮 enabled 条件（与 useAsrInput.canStartAsr 对齐）：
  // - auto_listen 模式开 + 总开关开：功能开关可用
  // - 总开关关 → 整体禁用（总开关是语音输入的总闸，手动 mic 一并关闭）
  const canStartMic = computed(
    () =>
      (autoListenOn.value && asrStore.settings.voice_input_enabled) ||
      asrInput.phase.value === "recording" ||
      asrInput.canStartAsr(false, true),
  );

  // 截图相关状态
  const hasScreenshot = ref(false);
  const screenshotBase64 = ref<string | null>(null);
  const isCapturing = ref(false);

  // 响应式容器宽度（窄屏判断从 uiStore 读取）
  const containerWidth = ref(60);

  const updateContainerWidth = () => {
    containerWidth.value = Math.max(60, uiStore.aspectRatio > 1 ? 70 : 90);
    isMobile.value = uiStore.aspectRatio <= 1;
    if (!isMobile.value) showMobileMenu.value = false;
  };

  const openSceneSettings = () => {
    uiStore.toggleSettings(true);
    uiStore.setSettingsTab("background");
  };

  // 移动端菜单操作：执行动作后自动收起菜单
  const onMobileMenuAction = (action: () => void) => {
    action();
    showMobileMenu.value = false;
  };
  const currentDisplayedText = ref("");

  // 逐字符淡入+上浮渲染器。颜色规则：
  // 内联模式 → \n 前为台词白字、\n 后为动作灰字；标准模式 → 两段式动作阶段整段灰字。
  const charReveal = createCharRevealWriter({
    charHtml: (char, index, rawText, animate) => {
      if (char === "\n") return "<br>";
      if (char === " ") return " ";
      let color = "#fff";
      if (settingsStore.text.inlineMotionText) {
        const newlineIndex = rawText.indexOf("\n");
        if (newlineIndex >= 0 && index > newlineIndex) color = "#9ca3af";
      } else if (isShowingMotionText.value) {
        color = "#9ca3af";
      }
      const anim = animate
        ? ";animation:tw-char-rise .28s cubic-bezier(.22, 1, .36, 1) forwards"
        : "";
      return `<span style="display:inline-block;color:${color}${anim}">${escapeHtml(char)}</span>`;
    },
  });

  // 清空回复显示区并重置渲染器增量状态（新台词 / 两段式动作阶段切换前调用）
  function resetResponseDisplay() {
    if (inlineDisplayRef.value) inlineDisplayRef.value.innerHTML = "";
    charReveal.reset();
  }

  // 立即把当前台词写入显示元素（不经过打字动画；供挂载恢复使用）
  function renderLineInstant(line: string) {
    currentDisplayedText.value = line;
    const text =
      settingsStore.text.inlineMotionText && uiStore.showCharacterMotionText
        ? line + "\n" + uiStore.showCharacterMotionText
        : line;
    if (inlineDisplayRef.value) charReveal.renderInstant(inlineDisplayRef.value, text);
  }

  // 回复显示区 TypeWriter（标准/内联模式共用；逐字符渲染由 charReveal 负责）
  const { startTyping, stopTyping, isTyping, finishTyping } = useTypeWriter(
    inlineDisplayRef,
    (text) => {
      currentDisplayedText.value = text;
    },
    charReveal.writeFn
  );

  const isSending = computed(() => gameStore.currentStatus === "thinking");

  // 标准模式两段式动作文本样式（颜色由逐字符 span 控制，这里只负责斜体与字号）
  const textareaMotionClass = computed(() => {
    if (!isShowingMotionText.value) return {};
    return { "italic text-base": true };
  });

  const emit = defineEmits(["player-continued", "dialog-proceed"]);

  const openHistory = () => {
    uiStore.toggleSettings(true);
    uiStore.setSettingsTab("history");
  };

  const handleRightClick = (e: MouseEvent) => {
    if (gameStore.command === "touch") {
      e.preventDefault();
      exitTouchMode();
    }
  };

  const handleDialogShow = (e: MouseEvent) => {
    if (isHidden.value) {
      e.preventDefault();
      isHidden.value = false;
    }
  };

  const toggleTouchMode = () => {
    if (gameStore.command === "touch") {
      exitTouchMode();
    } else {
      document.body.style.cursor = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' class='lucide lucide-hand-icon lucide-hand'%3E%3Cpath d='M18 11V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2'/%3E%3Cpath d='M14 10V4a2 2 0 0 0-2-2a2 2 0 0 0-2 2v2'/%3E%3Cpath d='M10 10.5V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2v8'/%3E%3Cpath d='M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-5.99-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15'/%3E%3C/svg%3E") 0 0, auto`;
      gameStore.command = "touch";
      document.addEventListener("contextmenu", handleRightClick);
    }
  };

  const exitTouchMode = () => {
    document.body.style.cursor = "default";
    gameStore.command = null;
    document.removeEventListener("contextmenu", handleRightClick);
  };

  const placeholderText = computed(() => {
    // 录音中：展示"正在聆听"（流式模式 partial 已实时写入输入框，此占位仅兜底非流式）
    if (asrInput.phase.value === "recording") {
      return t("game.dialog.listening");
    }

    switch (gameStore.currentStatus) {
      case "input":
        return uiStore.showPlayerHintLine || t("game.dialog.inputPlaceholder");
      case "thinking":
        const currentInteractRole = gameStore.currentInteractRole;
        if (currentInteractRole) {
          const baseMessage = currentInteractRole.thinkMessage;
          if (gameStore.thinkingLength > 0) {
            return `${baseMessage}${t("game.dialog.thinkingDepth", { count: gameStore.thinkingLength })}`;
          }
          return baseMessage;
        } else {
          return t("game.dialog.waitingResponse");
        }
      case "responding":
      case "presenting":
        return "";
      default:
        return t("game.dialog.inputPlaceholder");
    }
  });

  const isInputEnabled = computed(
    () => gameStore.currentStatus === "input" && !asrVoiceActive.value,
  );

  watch(
    () => gameStore.currentStatus,
    (newStatus) => {
      console.log("游戏状态变为 :", newStatus);
      if (newStatus === "thinking") {
        const currentInteractRole = gameStore.currentInteractRole;
        if (currentInteractRole) {
          //currentInteractRole.emotion = 'AI思考'
          uiStore.showCharacterTitle = currentInteractRole.roleName;
          uiStore.showCharacterSubtitle = currentInteractRole.roleSubTitle;
        }
      } else if (newStatus === "input") {
        uiStore.showCharacterTitle = gameStore.userName;
        uiStore.showCharacterSubtitle = gameStore.userSubtitle;
        uiStore.showCharacterEmotion = "";
      } else if (newStatus === "presenting") {
        uiStore.showCharacterTitle = "";
        uiStore.showCharacterSubtitle = "";
        uiStore.showCharacterEmotion = "";
        uiStore.showCharacterLine = "";
      }
    }
  );

  watch(
    [() => uiStore.showCharacterLine, () => gameStore.currentStatus],
    ([newLine, newStatus]) => {
      if (newLine && newLine !== "" && newStatus === "responding") {
        inputMessage.value = "";
        currentDisplayedText.value = "";
        isShowingMotionText.value = false;

        // 标准/内联模式统一渲染到回复 div（内联模式有动作文本时拼接换行+灰字）
        const text =
          settingsStore.text.inlineMotionText && uiStore.showCharacterMotionText
            ? newLine + "\n" + uiStore.showCharacterMotionText
            : newLine;
        resetResponseDisplay();
        startTyping(text, uiStore.typeWriterSpeed);
      } else if (newStatus === "input") {
        stopTyping();
        isShowingMotionText.value = false;
        inputMessage.value = "";
        currentDisplayedText.value = "";
      }
    }
  );

  // 回复 div 可见时自动聚焦，确保 Enter 键能推进对话（textarea 隐藏后这是唯一 Enter 入口）
  watch(currentStatus, (status) => {
    if (status === "responding") {
      // setTimeout 确保 v-show 已生效、DOM 已渲染
      setTimeout(() => inlineDisplayRef.value?.focus(), 0);
    }
  });

  // === 语音输入 toggle（useAsrInput 接管生命周期，替换上游 Web Speech 实现） ===
  async function toggleRecording() {
    try {
      // auto_listen 模式开 + 总开关开：mic 按钮 = 切换功能开关（暂停/恢复监听），
      // 不改模式设置；总开关关 → 走手动录音分支
      if (autoListenOn.value && asrStore.settings.voice_input_enabled) {
        asrInput.toggleAutoListenFunction();
        return;
      }
      if (asrInput.phase.value === "idle") {
        await asrInput.start("button");
      } else if (asrInput.phase.value === "recording") {
        asrInput.stop();
      }
    } catch (err) {
      console.warn("[ASR] toggle failed:", err);
    }
  }

  // 监听 asr-text 事件（useAsrInput fill_only 模式 dispatch）
  // fill_only 语义：识别结果填入 inputMessage，由用户手动发送（Enter / 发送按钮）。
  // 短暂显示锁仅防 auto_listen 立即再触发录音覆盖刚填入的内容（§1.10）。
  const ASR_DISPLAY_MS = 400;
  function onAsrText(e: Event) {
    const ce = e as CustomEvent<string>;
    if (typeof ce.detail === "string") {
      inputMessage.value = ce.detail;
      lockAsrForDisplay(ASR_DISPLAY_MS);
    }
  }

  // 监听 asr-send 事件（useAsrInput auto_send 模式 dispatch）：
  // 识别结果先显示到输入框，ASR_AUTO_SEND_DELAY_MS 后走 send()——
  // 完整复用剧本分支（runningScript → script_submit_input）、模型配置检查与
  // 输入框清理（显示锁已由 handle() 设置，这里不重复 lock）
  function onAsrAutoSend(e: Event) {
    const ce = e as CustomEvent<string>;
    if (typeof ce.detail !== "string") return;
    inputMessage.value = ce.detail;
    window.setTimeout(() => send(), ASR_AUTO_SEND_DELAY_MS);
  }

  let unlistenScreenshot: (() => void) | null = null;
  let unlistenCancelled: (() => void) | null = null;

  onMounted(async () => {
    // 模式切换重挂载：立即从 store 恢复当前台词（不重播打字动画）
    const restoreLine = uiStore.showCharacterLine;
    if (restoreLine && restoreLine !== "" && gameStore.currentStatus === "responding") {
      renderLineInstant(restoreLine);
    }

    document.addEventListener("contextmenu", handleDialogShow);
    // 监听 asr-text 事件（fill_only 模式 dispatch）
    window.addEventListener("asr-text", onAsrText);
    // 监听 asr-send 事件（auto_send 模式 dispatch）
    window.addEventListener("asr-send", onAsrAutoSend);
    // 输入框桥：流式 partial 实时写入 + 拼接基准读取
    registerAsrInputBridge({
      getText: () => inputMessage.value,
      setText: (v) => {
        inputMessage.value = v;
      },
    });
    // 初始化容器宽度
    updateContainerWidth();
    // 监听窗口大小变化
    window.addEventListener("resize", updateContainerWidth);

    // 监听截图完成事件
    unlistenScreenshot = await listen<{ base64: string }>("screenshot:captured", (event) => {
      screenshotBase64.value = event.payload.base64;
      hasScreenshot.value = true;
      isCapturing.value = false;
    });

    // 监听截图取消事件
    unlistenCancelled = await listen("screenshot:cancelled", () => {
      isCapturing.value = false;
      hasScreenshot.value = false;
    });
  });

  onUnmounted(() => {
    document.removeEventListener("contextmenu", handleDialogShow);
    window.removeEventListener("resize", updateContainerWidth);
    window.removeEventListener("asr-text", onAsrText);
    window.removeEventListener("asr-send", onAsrAutoSend);
    if (unlistenScreenshot) unlistenScreenshot();
    if (unlistenCancelled) unlistenCancelled();
  });

  async function startScreenshot() {
    if (isCapturing.value) return;
    isCapturing.value = true;
    try {
      await invoke("start_screenshot");
    } catch (error) {
      console.error("启动截图失败:", error);
      isCapturing.value = false;
      await dialogStore.alert(t("game.dialog.screenshotFailed"));
    }
  }

  function clearScreenshot() {
    if (hasScreenshot.value) {
      hasScreenshot.value = false;
      screenshotBase64.value = null;
    }
  }

  function sendOrContinue() {
    if (gameStore.currentStatus === "input") {
      send();
    } else if (gameStore.currentStatus === "responding") {
      continueDialog(true);
    }
  }

  function send() {
    const text = inputMessage.value;
    if (!text.trim()) return;

    // 检查对话模型是否已选择
    if (!llmStore.chatProviderId) {
      uiStore.showNotification({
        type: "warning",
        title: t("game.dialog.noModelTitle"),
        message: t("game.dialog.noModelMessage"),
        skipTipsCheck: true,
      });
      return;
    }

    gameStore.appendGameMessage({
      type: "message",
      displayName: gameStore.userName,
      content: text,
    });

    // In script mode, submit input to the script engine; otherwise use chat
    if (gameStore.runningScript) {
      const script = gameStore.runningScript;
      const wasChoice = script.choices.length > 0;
      // 只有提交成功才清空选项。以前是无条件清空的：allow_free 为 false 时后端
      // 会拒绝这次输入，而选项按钮已经消失、引擎仍在等待选择，玩家彻底卡死。
      invoke("script_submit_input", { input: text })
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
            title: wasChoice ? "请点击一个选项" : "当前无法输入",
            message: String(error),
            skipTipsCheck: true,
          });
        });
    } else {
      invoke("send_chat_message", {
        text,
        screenshotBase64: screenshotBase64.value,
      }).catch((error) => {
        console.error("发送消息失败:", error);
        gameStore.currentStatus = "input";
      });
    }

    // 发送后清除截图状态
    hasScreenshot.value = false;
    screenshotBase64.value = null;
    inputMessage.value = "";
  }

  function continueDialog(isPlayerTrigger: boolean): boolean {
    // 打字中：第一次点击跳过动画、显示完整文本（finish 已修复为补全剩余字符）
    if (isTyping.value) {
      finishTyping();
      return false; // 先跳到末尾，不推进
    }

    // 标准模式两段式动作文本
    if (!settingsStore.text.inlineMotionText) {
      // Phase 2: motion text already shown, advance normally
      if (isShowingMotionText.value) {
        isShowingMotionText.value = false;
        uiStore.showCharacterMotionText = "";
      }
      // Phase 1: there's pending motion text, show it instead of advancing
      else if (uiStore.showCharacterMotionText) {
        isShowingMotionText.value = true;
        resetResponseDisplay();
        startTyping(uiStore.showCharacterMotionText, uiStore.typeWriterSpeed);
        return false; // don't advance event queue
      }
    } else {
      uiStore.showCharacterMotionText = ""; // 内联模式动作文本已随台词显示，推进前清除
    }

    // Normal: advance to next event
    const needWait = eventQueue.continue();
    if (!needWait) {
      if (isPlayerTrigger) emit("player-continued");
      emit("dialog-proceed");
    }

    return needWait;
  }

  function removeDialog(_e: Event) {
    hide();
  }

  // ── 对话框外观（响应 settings store） ──
  // Dialog appearance logic extracted to composable: useDialogAppearance

  defineExpose({
    continueDialog,
    isTyping,
  });
</script>

<style scoped>
  /* AI 回复显示区：标准/内联模式共用；颜色由逐字符 span 内联样式控制 */
  .response-display {
    color: #9ca3af; /* fallback：极端情况下 div 直接显示文字时用灰色 */
  }

  /* 分割线：青蓝色微光点缀，亮段沿线条从左向右流动。
 * 视觉同源桌宠外框（青色 rgba(34,211,238)），但只保留微弱点缀。
 * 用 background-position 驱动而非 transform/子元素：背景只绘制在元素盒内，
 * 不会撑宽布局、也不会触发父级 overflow-x 滚动。 */
  .dialog-divider-glow {
    height: 1px;
    border-radius: 9999px;
    background-color: rgba(34, 211, 238, 0.12);
    background-image: linear-gradient(
      90deg,
      transparent 0%,
      rgba(34, 211, 238, 0.12) 30%,
      rgba(34, 211, 238, 0.55) 50%,
      rgba(34, 211, 238, 0.12) 70%,
      transparent 100%
    );
    background-size: 30% 100%;
    background-repeat: no-repeat;
    background-position: -30% 0;
    box-shadow: 0 0 2px rgba(110, 187, 199, 0.01);
    animation: dialog-divider-flow 3s linear infinite;
  }
  @keyframes dialog-divider-flow {
    from {
      background-position: -30% 0;
    }
    to {
      background-position: 130% 0;
    }
  }

  /* 兼容 Firefox */
  .custom-scroll {
    scrollbar-width: thin;
  }

  /* 兼容 Chrome / Edge / Safari */
  .custom-scroll::-webkit-scrollbar {
    width: 6px; /* 纵向滚动条宽度 */
    height: 6px; /* 横向滚动条高度（你这个是 overflow-x，主要控制这个） */
  }

  /* 移动端折叠按钮 — 与右侧 nav 关闭按钮等大 */
  .mobile-toggle-btn {
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.2);
    color: white;
    border-radius: 8px;
    padding: 10px 14px;
    cursor: pointer;
    font-size: 16px;
    line-height: 1;
    transition: all 0.25s ease;
    margin: 0 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 38px;
  }
  .mobile-toggle-btn:hover {
    background: rgba(255, 255, 255, 0.2);
    color: var(--accent-color, #6eb4ff);
  }
  .mobile-toggle-btn:active {
    transform: scale(0.92);
  }
  .mobile-toggle-btn > span,
  .mobile-toggle-btn {
    transition: transform 0.25s ease;
  }
  .mobile-toggle-btn.is-open {
    transform: rotate(180deg);
    background: rgba(255, 255, 255, 0.18);
    color: var(--accent-color, #6eb4ff);
    border-color: var(--accent-color, #6eb4ff);
  }

  /* 移动端下拉菜单 */
  .mobile-menu-dropdown {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 8px 4px 4px;
    margin-top: 2px;
    border-top: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(0, 14, 39, 0.5);
    border-radius: 0 0 8px 8px;
    width: 100%; /* 确保占满整个对话框宽度 */
  }

  /* Vue Transition: 移动端菜单展开/收起 */
  .mobile-menu-enter-active {
    animation: menu-slide-down 0.2s ease-out;
  }
  .mobile-menu-leave-active {
    animation: menu-slide-down 0.15s ease-in reverse;
  }
  @keyframes menu-slide-down {
    from {
      opacity: 0;
      max-height: 0;
      padding-top: 0;
      padding-bottom: 0;
      margin-top: 0;
      border-top-width: 0;
    }
    to {
      opacity: 1;
      max-height: 200px;
      padding-top: 8px;
      padding-bottom: 4px;
      margin-top: 2px;
      border-top-width: 1px;
    }
  }

  /* 情绪标签切换：上一个情绪向左滑出，下一个情绪从右侧滑入（推挤效果） */
  .emotion-slide-enter-active,
  .emotion-slide-leave-active {
    transition:
      transform 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94),
      opacity 0.3s ease;
  }
  /* 离开中的旧情绪脱离文档流，覆盖在新情绪上方向左滑出，
 * 容器宽度由新情绪决定，避免标题栏按钮被临时撑开 */
  .emotion-slide-leave-active {
    position: absolute;
    left: 0;
    top: 0;
  }
  .emotion-slide-enter-from {
    transform: translateX(100%);
    opacity: 0;
  }
  .emotion-slide-leave-to {
    transform: translateX(-100%);
    opacity: 0;
  }

  /* 标题栏（角色名 + 副标题）切换：整体从上方滑出/滑入（与情绪标签的左右滑动区分） */
  .title-slide-enter-active,
  .title-slide-leave-active {
    transition:
      transform 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94),
      opacity 0.3s ease;
  }
  /* 离开中的旧标题脱离文档流，向上滑出，避免撑动标题栏布局 */
  .title-slide-leave-active {
    position: absolute;
    left: 0;
    top: 0;
  }
  .title-slide-enter-from {
    transform: translateY(-100%);
    opacity: 0;
  }
  .title-slide-leave-to {
    transform: translateY(-100%);
    opacity: 0;
  }
</style>

<style>
  /* 底部 Home 指示器安全区：对话框本体铺到屏幕底（其半透明底盖住背景条带），
     仅内容底部让出 env() 高度，输入框不被 Home 指示器遮挡（桌面/Android 桌面 env=0） */
  .game-dialog {
    padding-bottom: calc(15px + var(--safe-area-inset-bottom, 0px));
  }
  /* 逐字符淡入+上浮动画。keyframes 必须全局：span 由 JS 动态生成，scoped 选择器无法命中 */
  @keyframes tw-char-rise {
    from {
      opacity: 0;
      transform: translateY(0.35em);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>

<template>
  <div class="main-box">
    <!-- 主界面始终渲染，加载动画期间在后台初始化 -->
    <FreeModeTools />
    <FullAccessWarning />
    <GameBackground></GameBackground>
    <!-- <GameAvatar ref="gameAvatarRef" @audio-ended="handleAudioFinished" />  -->
    <GameRolesStage
      ref="gameAvatarRef"
      @audio-ended="handleAudioFinished"
      @audio-started="handleAudioStarted"
    />
    <GameDialog ref="gameDialogRef" @player-continued="manualTriggerContinue" />

    <!-- 原有的菜单按钮 -->
    <div id="menu-panel" ref="menuPanelRef">
      <ToolActivityStatus v-if="!(gameStore.runningScript && gameStore.runningScript.isRunning)" />
      <Button
        type="nav"
        icon="play"
        @click="switchAutoMode"
        :active="uiStore.autoMode"
        v-show="uiStore.showSettings !== true"
      >
        <h3 class="hidden xl:block">{{ $t("views.mainChat.auto") }}</h3>
      </Button>
      <!-- 桌宠模式依赖 Windows 透明置顶窗口与 hit-test（lib.rs 为 cfg(windows)），Android 不可用 -->
      <Button
        v-if="!isAndroid()"
        type="nav"
        icon="character"
        @click="goToPetMode"
        v-show="uiStore.showSettings !== true"
      >
        <h3 class="hidden xl:block">{{ $t("views.mainChat.pet") }}</h3>
      </Button>
      <Button type="nav" icon="text" @click="openSettings" v-show="uiStore.showSettings !== true">
        <h3 class="hidden xl:block">{{ $t("views.mainChat.menu") }}</h3>
      </Button>
    </div>
    <GameExtraUI />

    <!-- Android 拍照 / 相册来源选择 sheet,见 useImageSourcePicker. 仅 chat 路由可见(PetMode 在手机上已停用) -->
    <ImageSourcePicker />

    <!-- 首次加载过渡动画（覆盖在主界面上方，主界面在后台并行初始化） -->
    <LoadingTransition v-if="showLoading" @complete="onLoadingComplete" />
  </div>
</template>

<script setup lang="ts">
  import FreeModeTools from "@/components/tools/FreeModeTools.vue";
  import ToolActivityStatus from "@/components/tools/ToolActivityStatus.vue";
  import { eventQueue } from "@/core/events/event-queue";
  import { getEnvConfigByKey } from "@/api/services/config";
  import { onMounted, ref, watch } from "vue";
  import { useRouter } from "vue-router";
  import { useGameStore } from "../../stores/modules/game";
  import { useUIStore } from "../../stores/modules/ui/ui";
  import { Button } from "../base";
  import { GameBackground, GameDialog, GameRolesStage } from "../game/standard";
  import LoadingTransition from "./LoadingTransition.vue";

  import FullAccessWarning from "@/components/tools/FullAccessWarning.vue";
  import ImageSourcePicker from "@/components/ui/ImageSourcePicker.vue";
  import { useHideForSnapshot } from "@/composables/useHideForSnapshot";
  import { useSettingsSnapshot } from "@/composables/useSettingsSnapshot";
  import { useSettingsStore } from "../../stores/modules/settings";
  import { isAndroid, isWindows } from "@/utils/platform";
  import GameExtraUI from "../game/standard/GameExtraUI.vue";

  const LOADING_STORAGE_KEY = "lingchat_loading_shown";

  // 会话级标记：同一页面 session 内只播放一次加载动画。
  // 仅靠 localStorage 会在路由卸载/重挂时回显（如桌宠切回聊天），
  // 用模块级变量兜底，确保一次启动只播放一次。
  let loadingShownThisSession = false;

  const router = useRouter();
  const uiStore = useUIStore();
  const gameStore = useGameStore();
  const settingsStore = useSettingsStore();

  // 首次加载过渡状态：仅当本次 session 未播放过且 localStorage 未标记时播放
  const showLoading = ref(!loadingShownThisSession && !localStorage.getItem(LOADING_STORAGE_KEY));

  function onLoadingComplete() {
    loadingShownThisSession = true;
    showLoading.value = false;
    localStorage.setItem(LOADING_STORAGE_KEY, "1");
    // 加载动画结束，恢复事件队列消费
    eventQueue.resume();
    // 通知 ASR：主界面加载完成，允许启动能量监测（§1.9）
    gameStore.setLoadingComplete(true);
  }

  // 高级设置可关闭首次开屏动画（display.disable_splash_animation）。
  // 关闭时直接进入完成态：跳过动画、恢复事件队列、放行 ASR。
  getEnvConfigByKey("display.disable_splash_animation")
    .then((setting) => {
      if (setting.value === "true" && showLoading.value) {
        onLoadingComplete();
      }
    })
    .catch(() => {
      // 读取失败（键不存在等）按默认行为播放开屏动画
    });

  const goToPetMode = () => {
    router.push("/pet");
  };

  const gameDialogRef = ref<InstanceType<typeof GameDialog> | null>(null);
  const menuPanelRef = ref<HTMLElement | null>(null);
  const settingsSnapshot = useSettingsSnapshot();
  const { hide: hideForSnapshot, restore: restoreForSnapshot } = useHideForSnapshot();
  let settingsSnapshotSession: number | null = null;

  const openSettings = async () => {
    // 存档截图（原逻辑，保留用于存档预览）
    gameStore.captureScreenshot();
    // Windows 静态背景快照 — 非阻塞：hide → capture → 立即开设置 → await → finally restore
    if (isWindows()) {
      const el = menuPanelRef.value;
      (async () => {
        let capturePromise: Promise<string | null> | null = null;
        try {
          await hideForSnapshot(el);
          capturePromise = settingsSnapshot.capture();
          uiStore.toggleSettings(true);
          uiStore.setSettingsTab("text");
          const result = await capturePromise;
          if (result) settingsSnapshotSession = settingsSnapshot.snapshotSessionId.value || null;
        } catch (e) {
          console.warn("[MainChat] settings snapshot failed:", e);
          // 失败也需打开设置，避免阻塞
          uiStore.toggleSettings(true);
          uiStore.setSettingsTab("text");
        } finally {
          restoreForSnapshot(el);
        }
        if (capturePromise) capturePromise.catch(() => restoreForSnapshot(el));
      })();
      return;
    }
    uiStore.toggleSettings(true);
    uiStore.setSettingsTab("text");
  };

  // 关闭设置后释放 Windows 静态背景临时资源
  watch(
    () => uiStore.showSettings,
    (show) => {
      if (!show && isWindows() && settingsSnapshotSession !== null) {
        const sid = settingsSnapshotSession;
        settingsSnapshotSession = null;
        settingsSnapshot.release(sid).catch(() => {});
      } else if (!show && isWindows() && settingsSnapshot.snapshotSrc.value) {
        // 兜底：MainMenu 未释放时由游戏侧释放
        settingsSnapshot.release().catch(() => {});
      }
    }
  );

  const switchAutoMode = () => {
    uiStore.autoMode = !uiStore.autoMode;
  };

  const runInitialization = async () => {
    try {
      await gameStore.initializeGame();
    } catch (error) {
      console.error("[MainChat] 初始化游戏失败:", error);
      uiStore.showWarning({ title: "初始化失败", message: "请尝试重新进入自由对话" });
    }
  };

  // 初始化游戏信息
  onMounted(() => {
    // 每次进入自由对话都恢复事件队列——编辑器试玩结束后 clear() 会把 paused 置 true，
    // 而 resume 只在首次加载的 LoadingTransition 里被调用，返回时走不到那里。
    // 但首次加载时不能在这里恢复：AI 开场白的打字机/音效必须等 LoadingTransition
    // 动画结束（onLoadingComplete 里 resume），否则会在开场动画遮罩后面提前播。
    if (!showLoading.value) {
      eventQueue.resume();
    }
    if (!gameStore.initialized) {
      runInitialization();
    }
  });

  /* 自动模式（AUTO）逻辑：事件驱动，非轮询
   * 当且仅当以下全部满足时，延迟 settingsStore.autoAdvanceDelay（默认 1000ms，可在设置→文字中调节）自动推进下一句：
   * 1. 自动模式开启
   * 2. 当前处于 responding 状态
   * 3. 当前台词打字机已结束
   * 4. 当前台词语音已播放完毕
   * 用户手动推进时取消当前调度。
   */

  const typingFinished = ref(true);
  const audioFinished = ref(true);
  let autoAdvanceTimer: ReturnType<typeof setTimeout> | null = null;

  const cancelAutoAdvance = () => {
    if (autoAdvanceTimer) {
      clearTimeout(autoAdvanceTimer);
      autoAdvanceTimer = null;
    }
  };

  const scheduleAutoAdvance = () => {
    cancelAutoAdvance();

    if (!uiStore.autoMode) return;
    if (gameStore.currentStatus !== "responding") return;
    if (!typingFinished.value || !audioFinished.value) return;

    autoAdvanceTimer = setTimeout(() => {
      autoAdvanceTimer = null;
      if (!uiStore.autoMode || gameStore.currentStatus !== "responding") return;
      if (!typingFinished.value || !audioFinished.value) return;

      const needWait = gameDialogRef.value?.continueDialog(false) ?? true;
      if (!needWait) {
        // 推进后重置状态，等待下一条台词的打字/语音事件
        typingFinished.value = true;
        audioFinished.value = true;
      }
    }, settingsStore.autoAdvanceDelay);
  };

  // 音频开始播放
  const handleAudioStarted = () => {
    audioFinished.value = false;
    cancelAutoAdvance();
  };

  // 音频播放结束
  const handleAudioFinished = () => {
    audioFinished.value = true;
    scheduleAutoAdvance();
  };

  // 用户手动推进
  const manualTriggerContinue = () => {
    cancelAutoAdvance();
  };

  // 监听自动模式开关
  watch(
    () => uiStore.autoMode,
    (enabled) => {
      if (enabled) scheduleAutoAdvance();
      else cancelAutoAdvance();
    }
  );

  // 监听游戏状态：进入 responding 时重置状态并等待事件
  watch(
    () => gameStore.currentStatus,
    (status) => {
      if (status === "responding") {
        typingFinished.value = !(gameDialogRef.value?.isTyping ?? false);
        audioFinished.value = true; // 新台词初始无音频
        scheduleAutoAdvance();
      } else {
        cancelAutoAdvance();
      }
    }
  );

  // 监听打字状态：结束立即尝试推进，开始则取消
  watch(
    () => gameDialogRef.value?.isTyping,
    (typing) => {
      if (typing) {
        typingFinished.value = false;
        cancelAutoAdvance();
      } else {
        typingFinished.value = true;
        scheduleAutoAdvance();
      }
    }
  );
</script>

<style>
  .main-box {
    position: absolute;
    height: 100%;
    width: 100%;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: center;
    overflow: hidden;
  }

  #menu-panel {
    display: flex;
    position: fixed;
    top: calc(15px + var(--safe-area-inset-top));
    right: 20px;
    z-index: 1000;
  }
  .scene-controls {
    position: fixed;
    bottom: 80px; /* 根据聊天输入框高度调整 */
    left: 20px;
    display: flex;
    gap: 8px;
    align-items: center;
    background: rgba(0, 0, 0, 0.5);
    padding: 8px 12px;
    border-radius: 20px;
    backdrop-filter: blur(5px);
    z-index: 100;
  }

  .scene-indicator {
    color: #fff;
    font-size: 14px;
    margin-left: 8px;
  }
</style>

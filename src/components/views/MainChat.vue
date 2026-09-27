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
      <!-- 桌面端：走原生窗口桌宠（/pet 路由 + set_pet_mode）。
           移动端：走 Android 系统级悬浮窗（floating-pet 插件），见 goToPetMode。 -->
      <Button
        type="nav"
        icon="character"
        @click="goToPetMode"
        :active="floatingPetActive"
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
import { getEnvConfigByKey } from "@/api/services/config";
import FreeModeTools from "@/components/tools/FreeModeTools.vue";
import ToolActivityStatus from "@/components/tools/ToolActivityStatus.vue";
import { eventQueue } from "@/core/events/event-queue";
import { onMounted, onUnmounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useGameStore } from "../../stores/modules/game";
import { useSettingsStore } from "../../stores/modules/settings";
import { useUIStore } from "../../stores/modules/ui/ui";
import { useDialogStore } from "../../stores/modules/ui/dialog";
import { Button } from "../base";
import { GameBackground, GameDialog, GameRolesStage } from "../game/standard";
import LoadingTransition from "./LoadingTransition.vue";

import FullAccessWarning from "@/components/tools/FullAccessWarning.vue";
import ImageSourcePicker from "@/components/ui/ImageSourcePicker.vue";
import { isMobile, isWindows } from "@/utils/platform";
import {
  getFloatingPetStatus,
  hideFloatingPet,
  isVisible,
  requestFloatingPetPermission,
  showFloatingPet,
} from "@/api/services/floating-pet";
import { useAutoAdvance } from "@/composables/chat/useAutoAdvance";
import GameExtraUI from "../game/standard/GameExtraUI.vue";

const LOADING_STORAGE_KEY = "lingchat_loading_shown";

// 会话级标记：同一页面 session 内只播放一次加载动画。
// 仅靠 localStorage 会在路由卸载/重挂时回显（如桌宠切回聊天），
// 用模块级变量兜底，确保一次启动只播放一次。
let loadingShownThisSession = false;

const router = useRouter();
const uiStore = useUIStore();
const dialogStore = useDialogStore();
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

/**
 * 进入/退出桌宠模式，按平台分流：
 *
 * - 桌面端：跳 `/pet` 路由，由 `set_pet_mode` 把窗口缩成透明置顶小窗
 * - Android：**先切 `/pet` 路由，再把主 WebView 搬进系统悬浮窗**
 *
 * ## 为什么必须先切页再搬移
 *
 * 悬浮窗里放的就是**主 WebView 本身**（不是新建实例），因此搬移是
 * 「瞬间完成、不重载页面」的。如果先搬移再切页，用户会看到主界面
 * 闪一下才变成桌宠；先切页则切页在屏幕内完成、搬移对用户无感。
 *
 * 悬浮窗路径的授权是「特殊权限」，无法运行时弹窗申请 —— 首次点击会跳系统
 * 设置页，用户授权返回后需再点一次。
 */
const floatingPetActive = ref(false);

/**
 * 用户被引导去系统设置页授权悬浮窗时置位。
 *
 * 回来后 `handleVisibilityChange` 会据此自动接着进桌宠——否则用户得自己
 * 再点一次「桌宠」按钮，而多数人不会知道要这么做。
 */
const pendingPetEntry = ref(false);

/** 同步悬浮窗状态。从系统设置页返回时刷新（按钮高亮与否）。 */
const syncFloatingPetState = async () => {
  if (!isMobile()) return;
  try {
    floatingPetActive.value = await isVisible();
  } catch {
    floatingPetActive.value = false;
  }
};

/**
 * 退出悬浮桌宠：把 WebView 搬回 Activity。
 *
 * 页面此时停在 `/pet`，搬回后要再切回 `/chat`，行为与桌面端一致
 * （桌面端 `set_pet_mode(false)` 之后同样 `router.push("/chat")`）。
 */
const leavePetMode = async () => {
  try {
    await hideFloatingPet();
    floatingPetActive.value = false;
    if (router.currentRoute.value.path === "/pet") {
      await router.push("/chat");
    }
  } catch (e) {
    console.error("[MainChat] 收回悬浮桌宠失败:", e);
  }
};

const goToPetMode = async () => {
  if (!isMobile()) {
    router.push("/pet");
    return;
  }

  // 已开启则关闭：给用户一个明确的退出路径，避免悬浮窗无法收回
  if (floatingPetActive.value) {
    await leavePetMode();
    uiStore.showInfo({ title: "桌宠已收回", message: "已切回主界面。" });
    return;
  }

  try {
    // 先探测再切页：没授权/不支持时不该让用户白跳一次 /pet 路由。
    const status = await getFloatingPetStatus();

    if (!status.supported) {
      uiStore.showWarning({
        title: "当前设备不支持",
        message: "这台设备的系统不允许创建悬浮窗，桌宠暂时无法使用。",
      });
      return;
    }

    if (!status.granted) {
      // 先解释、再跳转。
      //
      // 原来这里是「直接跳系统设置页 + showInfo 提示」，但那个 toast 是在
      // 应用**已经切到后台**之后才弹的，用户正盯着系统设置页，根本看不到——
      // 表现就是「点了桌宠什么都没发生」。
      //
      // 改成模态确认框：用户看清要开哪个开关、点确定才跳，返回后自动继续。
      const confirmed = await dialogStore.confirm(
        "桌宠需要「显示在其他应用上层」权限，才能浮在别的应用之上。\n\n" +
          "点「确定」会打开系统设置页，请在列表里找到 LingChat 并打开该开关。\n" +
          "返回本应用后会自动继续，不用再点一次桌宠。",
        "需要悬浮窗权限",
      );
      if (!confirmed) return;

      pendingPetEntry.value = true;
      await requestFloatingPetPermission();
      return;
    }

    // ① 先切页 —— 此时还在屏幕内，用户看到 /pet 渲染完成
    await router.push("/pet");
    // ② 再搬移 —— 原生把 WebView 摘进悬浮窗，Activity 换成占位页
    await showFloatingPet({ scale: settingsStore.pet?.scale ?? 1 });
    floatingPetActive.value = true;
  } catch (e) {
    console.error("[MainChat] 启动悬浮桌宠失败:", e);
    // 搬移失败时把页面退回来，避免用户停在 /pet 却不在悬浮窗里
    if (router.currentRoute.value.path === "/pet") {
      await router.push("/chat").catch(() => {});
    }
    uiStore.showError({
      title: "桌宠启动失败",
      message: "悬浮窗没能创建成功，请检查是否已授予悬浮窗权限。",
    });
  }
};

// 用户去系统设置授权后返回、或从悬浮窗切回 App，重新同步按钮状态
const handleVisibilityChange = async () => {
  if (document.visibilityState !== "visible") return;
  await syncFloatingPetState();

  // 刚才是为了授权才跳走的：回来后自动接着进桌宠，不用再点一次
  if (!pendingPetEntry.value) return;
  pendingPetEntry.value = false;

  const status = await getFloatingPetStatus();
  if (status.granted) {
    await goToPetMode();
    return;
  }

  // 没授权成功就别静默失败——用户很可能在系统设置里没找到那个开关
  uiStore.showWarning({
    title: "还没有拿到权限",
    message:
      "LingChat 的「显示在其他应用上层」开关仍是关闭的，桌宠无法启动。\n" +
      "部分系统里这个开关叫「悬浮窗」或「后台弹出界面」。",
    duration: 8000,
  });
};

onMounted(() => {
  if (!isMobile()) return;
  void syncFloatingPetState();
  document.addEventListener("visibilitychange", handleVisibilityChange);
});

onUnmounted(() => {
  document.removeEventListener("visibilitychange", handleVisibilityChange);
});

const gameDialogRef = ref<InstanceType<typeof GameDialog> | null>(null);
const menuPanelRef = ref<HTMLElement | null>(null);
let settingsSnapshotSession: number | null = null;

const openSettings = async () => {
  // 存档截图（原逻辑，保留用于存档预览）
  gameStore.captureScreenshot();
  // Windows 静态背景快照 — 非阻塞：hide → capture → 立即开设置 → await → finally restore
  if (isWindows()) {
    const el = menuPanelRef.value;
    (async () => {
      try {
        uiStore.toggleSettings(true);
        uiStore.setSettingsTab("text");
      } catch (e) {
        console.warn("[MainChat] settings snapshot failed:", e);
        // 失败也需打开设置，避免阻塞
        uiStore.toggleSettings(true);
        uiStore.setSettingsTab("text");
      } finally {
      }
    })();
    return;
  }
  uiStore.toggleSettings(true);
  uiStore.setSettingsTab("text");
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

// 自动推进调度（AUTO + 台词合并共用一条管道）—— 与桌宠 PetMode 共用同一实现
const {
  onAudioStarted: handleAudioStarted,
  onAudioFinished: handleAudioFinished,
  manualTriggerContinue,
  toggleAutoMode: switchAutoMode,
} = useAutoAdvance({
  dialog: () => gameDialogRef.value,
  mergeEnabled: true,
});
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

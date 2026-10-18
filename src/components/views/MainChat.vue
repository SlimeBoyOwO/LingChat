<template>
  <div class="main-box">
    <!-- 主界面始终渲染，加载动画期间在后台初始化 -->
    <FreeModeTools />
    <div class="main-status-panel">
      <div class="main-status-row">
        <StatusInfoBar />
      </div>
      <FullAccessWarning />
    </div>
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
        @click="goToPetMode()"
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
import StatusInfoBar from "@/components/tools/StatusInfoBar.vue";
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
  isScreenCaptureReady,
  onPetPermissionDenied,
  requestFloatingPetPermission,
  requestScreenCapture,
  showFloatingPet,
  waitForScreenCapture,
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

/**
 * 本次「进桌宠」尝试里，屏幕截取授权已经问过一次了。
 *
 * 用来避免死循环：用户在系统授权框上点了取消 → 回到前台 →
 * `handleVisibilityChange` 自动重试 `goToPetMode` → 又发现没授权 → 又弹说明……
 * 置位之后第二次只给一句 toast 收尾，不再重复弹框。
 *
 * 进桌宠成功、或用户主动重新点按钮时清零（见 `goToPetMode` 开头）。
 */
const screenCaptureAsked = ref(false);

/**
 * 权限被拒时桌宠的台词。
 *
 * 用**应用内 toast**（`uiStore.showWarning`）而不是系统 Toast：系统气泡是
 * 灰底黑字，跟 App 视觉完全脱节。这几刻页面都是可用的，所以走页面自己弹。
 *
 * 每一项权限都有**两条**拒绝路径，二者文案必须一致，否则同一件事会出现
 * 两种说法：
 * - 系统侧：去设置页没打开开关 / 授权框上点取消，原生派发
 *   `pet-permission-denied`
 * - 应用侧：说明弹窗上点了「就不去」「偏不给看」
 */
const notifyPermissionDenied = (which: "overlay" | "screen") => {
  uiStore.showWarning(
    which === "overlay"
      ? {
          title: "哼，小气鬼！",
          message: "不让我出去，那你遇到问题可别眼巴巴地来找我。",
          duration: 4000,
        }
      : {
          title: "防贼一样！",
          message: "不看就不看，待会儿你无聊了，可别怪我没理你。",
          duration: 4000,
        },
  );
};

/** `onPetPermissionDenied` 的取消函数，卸载时释放。 */
let unlistenPermissionDenied: (() => void) | null = null;

/**
 * 同步悬浮窗状态。从系统设置页返回时刷新（按钮高亮与否）。
 *
 * ## 为什么不用 `isVisible()`
 *
 * 它底层是裸 `invoke`，失败就抛，调用方只能 catch 成 `false`——
 * 而 `false` 在这里会被读作「桌宠没在运行」。一次 IPC 失败（搬运 / 重排
 * 期间主线程忙）就会让按钮变回「启动」，用户再点一下，**正在悬浮窗里跑**
 * 的桌宠会被重新搬一次（窗口重建、位置重置到 (0,0)、角色闪没）。
 *
 * 聚合查询带 `queried` 标记，能把「查不到」和「确实没在运行」分开：
 * 查不到就保持上一次的按钮状态，不误判。
 */
const syncFloatingPetState = async () => {
  if (!isMobile()) return;
  const status = await getFloatingPetStatus();
  if (!status.queried) return;
  floatingPetActive.value = status.visible;
};

/**
 * 兜底自愈：修掉「页面不在 `/pet`，但 WebView 还在悬浮窗里」这个非法状态。
 *
 * ## 它只可能由一次误判造成
 *
 * 悬浮窗里应该**永远**是桌宠页。这个状态只可能来自 `PetMode` 里一次
 * 「已回到 App」的误判（见 `confirmReturnedToApp`）—— 页面被 push 到了
 * `/chat`，原生窗口却还挂着。用户看到的就是「悬浮窗卡成聊天页、角色凭空
 * 消失」，而且因为那次误判顺带停掉了轮询，它**永远不会自愈**。
 *
 * ## 为什么要延迟 + 二次确认
 *
 * **不能**一读到 `detached === true` 就跳。正常收回（点 ✕）时也会短暂出现
 * 完全一样的读数：`PetMode` 是「先 `handleReturnedToApp()` 切路由、再
 * `hideFloatingPet()`」，而原生的 `hide` 命令内部还有 `HIDE_DELAY_MS`(80ms)
 * 的延迟。这段窗口里 MainChat 已经挂载、页面已是 `/chat`，而原生还没摘窗口
 * —— 此时若立刻跳回 `/pet`，用户会看到「刚收回又被弹回桌宠页」。
 *
 * 因此等一拍（1.5s，远大于 80ms + IPC 往返）再查一次：原生确实还在悬浮窗里
 * 才补导航。
 */
const healStuckFloatingState = async () => {
  if (router.currentRoute.value.path === "/pet") return;
  const status = await getFloatingPetStatus();
  // 查不到就不动；已经收回了也正常，什么都不用做
  if (!status.queried || !status.detached) return;
  await router.replace("/pet").catch(() => {});
};

/** [healStuckFloatingState] 的延迟时长（毫秒）。见其说明。 */
const STUCK_HEAL_DELAY_MS = 1500;

/**
 * 退出悬浮桌宠：把 WebView 搬回 Activity。
 *
 * 页面此时停在 `/pet`，搬回后要再切回 `/chat`，行为与桌面端一致
 * （桌面端 `set_pet_mode(false)` 之后同样 `router.replace("/chat")`）。
 */
const leavePetMode = async () => {
  try {
    await hideFloatingPet();
    floatingPetActive.value = false;
    if (router.currentRoute.value.path === "/pet") {
      await router.replace("/chat");
    }
  } catch (e) {
    console.error("[MainChat] 收回悬浮桌宠失败:", e);
  }
};

/**
 * @param opts.waitScreenReady 是否等待截屏授权落地。只由
 *   `handleVisibilityChange`（刚从系统授权框回来、自动续跑）传 true ——
 *   门禁预检不能等，「没就绪」本来就是这一轮的正常状态。
 */
const goToPetMode = async (opts?: { waitScreenReady?: boolean }) => {
  if (!isMobile()) {
    // `/pet` 与 `/chat` 是**同一个界面的两种形态**，不是「上一页 / 下一页」。
    // 用 replace 而不是 push：Android 的返回键由 WryActivity 交给 WebView
    // 历史栈（`mWebView.goBack()`），push 会让每切一次形态都多压一条历史 ——
    // 用户按返回就会在 /chat 和 /pet 之间来回弹（见 docs 5.4）。
    router.replace("/pet");
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

    // 查询失败：既不能当成「不支持」（会误报设备不兼容），也不能当成
    // 「没在运行」（会去重复搬一次）。老实说没问到，让用户再点一次。
    if (!status.queried) {
      uiStore.showWarning({
        title: "刚刚没问清楚呢…",
        message: "系统这会儿没回我话，过一会儿再点一次试试？",
      });
      return;
    }

    if (!status.supported) {
      uiStore.showWarning({
        title: "这台设备帮不上忙呢",
        message: "系统的限制下我暂时浮不起来，换个设备再喊我吧～",
      });
      return;
    }

    // 已经在悬浮窗里了（按钮状态可能过期，见 syncFloatingPetState）。
    // 这时**绝不能**再 show 一次：`show()` 的幂等分支会把窗口整个重建
    // ——位置重置到 (0,0)、WebView 重新挂载，用户看到的就是「角色闪一下
    // 没了、桌宠跳到左上角」。只需要把页面送回 /pet。
    if (status.detached) {
      floatingPetActive.value = true;
      if (router.currentRoute.value.path !== "/pet") {
        await router.replace("/pet");
      }
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
        "连个「显示在其他应用上层」的权限都没准备，我怎么飘在桌面上陪你？\n" +
          "快点去开启啦，我还等着出去透透气，顺便看看你有没有在摸鱼呢！",
        "喂，别把我关在后台！",
        { okText: "这就去开", cancelText: "就不去" },
      );
      if (!confirmed) {
        notifyPermissionDenied("overlay");
        return;
      }

      pendingPetEntry.value = true;
      await requestFloatingPetPermission();
      return;
    }

    // ★ 屏幕截取权限：与悬浮窗权限**同级**，两项齐备才允许进桌宠。
    //
    // 为什么把它设成硬门禁而不是「进不去也能用」：
    // 桌宠在手机上的截图键是「直接抓一屏」，没有授权就必然失败 ——
    // 用户看到的是「功能坏了」，而不是「我没给权限」。与其让他进去再撞墙，
    // 不如在门口就说清楚。
    //
    // 这里**只在续跑路径上等**授权落地：门禁预检时「没就绪」是正常状态，
    // 等它就是白等 1.2 秒（真机实测点击到弹窗 ≈1.4s）。
    const screenReady = opts?.waitScreenReady
      ? await waitForScreenCapture()
      : await isScreenCaptureReady();
    if (!screenReady) {
      // 已经问过一次、回来仍然没有：不再重复弹框，静默收尾。
      // 否则「取消 → 自动重试 → 又弹框」会变成死循环。
      //
      // 这里**不弹 toast**：用户在系统授权框上点取消时，原生
      // `MainActivity.onActivityResult` 已经派发过 `pet-permission-denied`，
      // 那边弹过一句了。这里再弹就是双份。
      if (screenCaptureAsked.value) {
        screenCaptureAsked.value = false;
        return;
      }

      const confirmed = await dialogStore.confirm(
        "以为我很想盯着你的屏幕看吗？不看清你在干嘛，我怎么知道什么时候该找你搭话？\n" +
          "放心，画面绝不外传。快点啦，立刻马上给我点确定！",
        "笨蛋，快给我权限啦！",
        { okText: "勉强同意吧", cancelText: "偏不给看" },
      );
      if (!confirmed) {
        notifyPermissionDenied("screen");
        return;
      }

      const reason = await requestScreenCapture();
      if (reason === "unsupported") {
        screenCaptureAsked.value = false;
        uiStore.showWarning({
          title: "这台设备帮不上忙呢",
          message: "系统的限制下我看不到屏幕，桌宠就先不出来啦～",
        });
        return;
      }
      if (reason === "error") {
        screenCaptureAsked.value = false;
        uiStore.showWarning({
          title: "刚刚没问清楚呢…",
          message: "系统这会儿没回我话，过一会儿再点一次试试？",
        });
        return;
      }
      if (reason === "prompted") {
        // 授权框是异步的，结果只落在原生侧。这里不能干等 ——
        // 用户回来后由 handleVisibilityChange 自动接着进桌宠。
        screenCaptureAsked.value = true;
        pendingPetEntry.value = true;
        return;
      }
      // already_granted：授权其实已经有了，继续往下走
      screenCaptureAsked.value = false;
    }

    // ① 先切页 —— 此时还在屏幕内，用户看到 /pet 渲染完成
    await router.replace("/pet");
    // ② 再搬移 —— 原生把 WebView 摘进悬浮窗，Activity 换成占位页
    await showFloatingPet({ scale: settingsStore.pet?.scale ?? 1 });
    floatingPetActive.value = true;
    // 进门成功，把「问过一次」的记号清掉：下次重新进（例如进程重启后
    // MediaProjection 授权丢失）还要能再问一遍。
    screenCaptureAsked.value = false;
  } catch (e) {
    console.error("[MainChat] 启动悬浮桌宠失败:", e);
    // 搬移失败时把页面退回来，避免用户停在 /pet 却不在悬浮窗里
    if (router.currentRoute.value.path === "/pet") {
      await router.replace("/chat").catch(() => {});
    }
    uiStore.showError({
      title: "桌宠没能爬出来…",
      message: "悬浮窗没建起来，看看「显示在其他应用上层」的开关是不是给到我啦？",
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
    // 续跑：此刻 token 多半还没换好，要等它落地，否则会被判成「没授权」。
    await goToPetMode({ waitScreenReady: true });
    return;
  }

  // 开关没打开就不做别的：原生 onResume 已经派发过 `pet-permission-denied`，
  // 由 notifyPermissionDenied 统一弹那一句。这里再弹一条就是双份提示。
};

onMounted(() => {
  if (!isMobile()) return;
  void syncFloatingPetState();
  // 兜底自愈：等一拍再查「页面不在 /pet 但 WebView 还在悬浮窗」这个非法状态。
  // 必须延迟 —— 正常收回时也会短暂出现同样的读数，见 healStuckFloatingState。
  // 挂在 onMounted 而不是 syncFloatingPetState 里：误判之后页面会被 push 到
  // /chat，那一刻正是本组件挂载的时候。
  window.setTimeout(() => void healStuckFloatingState(), STUCK_HEAL_DELAY_MS);
  document.addEventListener("visibilitychange", handleVisibilityChange);
  // 原生在「用户拒了悬浮窗权限 / 截屏授权」时派发这个 DOM 事件，
  // 由页面弹应用内 toast（不用系统 Toast，见 notifyPermissionDenied）。
  unlistenPermissionDenied = onPetPermissionDenied(notifyPermissionDenied);
});

onUnmounted(() => {
  document.removeEventListener("visibilitychange", handleVisibilityChange);
  unlistenPermissionDenied?.();
  unlistenPermissionDenied = null;
});

const gameDialogRef = ref<InstanceType<typeof GameDialog> | null>(null);
const menuPanelRef = ref<HTMLElement | null>(null);

const openSettings = async () => {
  // 存档截图（原逻辑，保留用于存档预览）
  gameStore.captureScreenshot();
  // Windows 静态背景快照 — 非阻塞：hide → capture → 立即开设置 → await → finally restore
  if (isWindows()) {
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

.main-status-panel {
  position: fixed;
  top: calc(15px + var(--safe-area-inset-top));
  left: 50%;
  z-index: 1001;
  display: flex;
  width: min(44rem, calc(100vw - 2rem));
  transform: translateX(-50%);
  flex-direction: column;
  align-items: center;
  gap: 8px;
  pointer-events: none;
}

.main-status-row {
  display: flex;
  /* 窄屏隐藏时间、天气时，仍为两侧导航按钮保留第一行。 */
  min-height: 48px;
  align-items: center;
  justify-content: center;
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

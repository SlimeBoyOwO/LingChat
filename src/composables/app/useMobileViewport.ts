/**
 * 移动端键盘 / 视觉视口适配 Composable（Android / iOS）
 *
 * 键盘弹出时把可见高度并入 --safe-area-inset-bottom（存在 .pb-safe/pb-safe-gap、
 * 对话框 padding、MusicPlayer 等 var() 用法，UI 自动上移让位）。
 * WKWebView 固定布局下聚焦输入框弹出键盘时，布局视口不会自动收缩，页面比可视区
 * 高 → 整个 webview 可上下滑动、输入框被键盘盖住；这里跟随 visualViewport
 * （键盘弹出=可视区高度），配合 index.html 的 interactive-widget=resizes-content
 * 双保险。
 *
 * 同时兜底禁用 WebView 原生缩放（双指捏合 / 双击放大）：index.html 的 viewport
 * meta（user-scalable=no + maximum-scale=1）与 base.css 的 touch-action: manipulation
 * 为主，这里再拦截 WebKit 缩放手势与多触点手势，确保 Android WebView / iOS WKWebView
 * 都无法捏合或双击放大界面。
 *
 * 仅移动端挂载：桌面端 visualViewport == window，这套逻辑是无操作死代码。
 * 在 App.vue 中调用一次。
 */
import { onMounted, onUnmounted } from "vue";
import { isMobile } from "@/utils/platform";

export function useMobileViewport() {
  const vv = window.visualViewport;
  // 基准底部安全区（无键盘时的 env 值，首个值即基线）
  let safeBaseBottom = 0;
  let safeBaseInitialized = false;
  // 当前已施加的抬升量（几何解算用自然位置 = 当前底部 + 已抬升量，避免自引用震荡）
  let currentLift = 0;
  // 键盘状态兜底轮询：外部/配件键盘等场景 vv 事件偶发不触发，
  // 轮询 vv.height 变化触发重算（800ms 一次，开销可忽略）
  let kbGuardTimer: ReturnType<typeof setInterval> | null = null;
  let lastKbSig = 0;

  const lockScroll = () => {
    window.scrollTo(0, 0);
    if (document.documentElement.scrollTop) document.documentElement.scrollTop = 0;
    if (document.body.scrollTop) document.body.scrollTop = 0;
  };

  // 页面级平移拦截（iOS 键盘收起后剩余的可滚动区）：根级 touchmove 直接 preventDefault，
  // 内部滚动容器（聊天记录/设置页等 overflow-* / custom-scroll）不受影响
  const preventRootTouchScroll = (e: TouchEvent) => {
    const t = e.target as HTMLElement | null;
    if (
      t &&
      t.closest(
        ".overflow-y-auto, .overflow-x-auto, .overflow-auto, .overflow-y-scroll, .overflow-x-scroll, .overflow-scroll, .custom-scroll, .scrollbar-thin, [data-scrollable]",
      )
    ) {
      return;
    }
    e.preventDefault();
  };

  const preventZoomGestures = (e: Event) => {
    // iOS Safari/WKWebView 的捏合缩放会触发 gesturestart/change/end，preventDefault 可取消缩放
    if (e.type === "gesturestart" || e.type === "gesturechange" || e.type === "gestureend") {
      e.preventDefault();
      return;
    }
    // 触点 >= 2 即是双指捏合手势（Android/通用），preventDefault 取消缩放
    const te = e as TouchEvent;
    if (te.touches && te.touches.length > 1) {
      e.preventDefault();
    }
  };

  const syncVisualViewport = () => {
    if (!vv || !isMobile()) return;
    const root = document.documentElement;
    if (!safeBaseInitialized) {
      // 初始同步：读取当前 env() 解析值作为基线（iOS: 34px 左右；桌面 0）
      const cur =
        parseFloat(getComputedStyle(root).getPropertyValue("--safe-area-inset-bottom")) || 0;
      safeBaseBottom = Number.isFinite(cur) ? cur : 0;
      safeBaseInitialized = true;
    }

    // iPad 检测：现代 iPadOS 为桌面版 UA（MacIntel + 多点触控）
    const isIPad =
      navigator.userAgent.includes("iPad") ||
      (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);

    // 键盘可见高度 = visualViewport 高度差（iOS 软键盘/配件条收缩量；
    // env(keyboard-inset-height) 无法通过 getComputedStyle 读取，只信 vv 差值）
    const kbd = window.innerHeight - vv.height;

    // 键盘可见区顶部（可见区高度 = min(vv.height, 窗口 - 键盘高度)）
    const visibleHeight = Math.min(vv.height, window.innerHeight - kbd);

    // 不让位的情形：无键盘；或 iPad 悬浮小键盘（可拖动，挡到输入框用户会自行移开）
    if (kbd <= 20 || (isIPad && kbd < 200)) {
      currentLift = 0;
    } else {
      // 几何解算让位：以「聚焦输入框自然底部 + 间距」相对键盘上方可见区的高度差为准。
      // 自然位置 = 当前底部 + 已施加抬升量（移除抬升影响），公式稳定不震荡。
      const ae = document.activeElement as HTMLElement | null;
      if (ae && typeof ae.getBoundingClientRect === "function") {
        const r = ae.getBoundingClientRect();
        const naturalBottom = r.bottom + currentLift;
        if (naturalBottom - 16 > visibleHeight) {
          currentLift = Math.max(0, Math.round(naturalBottom - 16 - visibleHeight));
        }
      }
    }
    // 仿 Android：底部安全区 = 基线 + 抬升量
    root.style.setProperty("--safe-area-inset-bottom", `${safeBaseBottom + currentLift}px`);

    // 页面始终锚定原点（键盘弹出的系统 focus-scroll 与手势滚动都会被锁回）
    lockScroll();
  };

  // 旋转/分屏后安全区基线失效：iPhone 竖屏底部 inset ≈34px、横屏 ≈21px（灵动岛移到左右），
  // iPad 台前调度改窗口尺寸同理。挂载时采样的基线在旋转后是旧值，这里强制重采样：
  //   1. 先清掉本模块写入的内联覆盖（iOS 回落 :root 的 env() 实时解析新值；
  //      Android 的 --safe-area-inset-* 由 MainActivity insets 监听注入，旋转后监听
  //      会重新触发注入，此处的临时清空无影响）
  //   2. 重置基线标记，下一次 sync 重读 env() 解析值作为新基线
  const handleOrientationChange = () => {
    document.documentElement.style.removeProperty("--safe-area-inset-bottom");
    safeBaseInitialized = false;
    currentLift = 0;
    syncVisualViewport();
  };

  onMounted(() => {
    // 仅移动端挂载：桌面 visualViewport == window，此处全为无操作死代码
    if (!isMobile()) return;

    if (vv) {
      vv.addEventListener("resize", syncVisualViewport);
      vv.addEventListener("scroll", syncVisualViewport);
    }
    window.addEventListener("orientationchange", handleOrientationChange);
    // 硬锁滚动：任何滚动（键盘 focus-scroll / 手势）立即归零
    window.addEventListener("scroll", lockScroll, { passive: true, capture: true });
    document.addEventListener("touchmove", preventRootTouchScroll, { passive: false });
    // 兜底：禁用双指/双击/捏合的原生缩放
    document.addEventListener("gesturestart", preventZoomGestures, { passive: false });
    document.addEventListener("gesturechange", preventZoomGestures, { passive: false });
    document.addEventListener("gestureend", preventZoomGestures, { passive: false });
    document.addEventListener("touchstart", preventZoomGestures, { passive: false });
    document.addEventListener("touchmove", preventZoomGestures, { passive: false });
    // 聚焦变化 → 重算让位（focusin 先清 0 由 vv resize 收敛，focusout 归零）
    document.addEventListener("focusin", syncVisualViewport, true);
    document.addEventListener("focusout", syncVisualViewport, true);
    if (vv) {
      // 兜底轮询：外部/配件键盘等场景 vv 事件偶发不触发，轮询高度变化重算
      kbGuardTimer = setInterval(() => {
        const sig = Math.round(vv.height);
        if (sig !== lastKbSig) {
          lastKbSig = sig;
          syncVisualViewport();
        }
      }, 800);
    }
    lockScroll();
  });

  onUnmounted(() => {
    if (!isMobile()) return;

    window.removeEventListener("orientationchange", handleOrientationChange);
    window.removeEventListener("scroll", lockScroll, { capture: true } as any);
    document.removeEventListener("touchmove", preventRootTouchScroll);
    document.removeEventListener("gesturestart", preventZoomGestures);
    document.removeEventListener("gesturechange", preventZoomGestures);
    document.removeEventListener("gestureend", preventZoomGestures);
    document.removeEventListener("touchstart", preventZoomGestures);
    document.removeEventListener("touchmove", preventZoomGestures);
    document.removeEventListener("focusin", syncVisualViewport, true);
    document.removeEventListener("focusout", syncVisualViewport, true);
    if (kbGuardTimer) {
      clearInterval(kbGuardTimer);
      kbGuardTimer = null;
    }
    if (vv) {
      vv.removeEventListener("resize", syncVisualViewport);
      vv.removeEventListener("scroll", syncVisualViewport);
    }
  });
}

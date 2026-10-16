/**
 * Ctrl+滚轮 全局 UI 缩放 Composable
 *
 * 按住 Ctrl 并滚动鼠标滚轮来缩放整个 UI 界面。
 * 使用 `transform: scale` + 尺寸补偿在 #app 元素上实现均匀缩放：
 * #app 布局尺寸设为 `calc(100dvw / z) × calc(100dvh / z)` 再整体放大 z 倍，
 * 视觉上恒精确填满窗口 —— 旧实现用 CSS zoom 直接缩放 100vw×100vh，
 * 放大时内容溢出视口被裁切（像放大镜）、缩小时填不满窗口留下空白。
 * 缩放级别保存在 localStorage 中，跨会话持久化。
 *
 * 在 App.vue 中调用一次以激活全局缩放功能。
 */

import { onUnmounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUIStore } from "@/stores/modules/ui/ui";
import { i18n } from "@/locales";
import { isWindows } from "@/utils/platform";

const ZOOM_STORAGE_KEY = "lingchat-ui-zoom";
const ZOOM_STEP = 0.05;
const ZOOM_MIN = 0.5;
const ZOOM_MAX = 2.0;
const ZOOM_DEFAULT = 1.0;
const ZOOM_DECIMALS = 2;

/** 防抖 toast 的最小间隔（毫秒），避免滚轮时频繁弹出通知 */
const TOAST_DEBOUNCE_MS = 200;

let lastToastTime = 0;

/**
 * 当前缩放级别（模块级共享 ref）。
 *
 * 放在 composable 外是为了让其他模块（如 EditorHeader 的 Tab 指示条）能读取
 * 当前缩放值做坐标换算 —— transform 方案下 getBoundingClientRect 返回视觉坐标
 * 而 scrollLeft 等是布局坐标，混用必须除以缩放值。
 */
export const currentZoom = ref<number>(loadZoom());

/** 读取持久化的缩放值 */
function loadZoom(): number {
  try {
    const stored = localStorage.getItem(ZOOM_STORAGE_KEY);
    if (stored) {
      const value = parseFloat(stored);
      if (!isNaN(value) && value >= ZOOM_MIN && value <= ZOOM_MAX) {
        return Math.round(value * 100) / 100;
      }
    }
  } catch {
    // localStorage 不可用时静默回退
  }
  return ZOOM_DEFAULT;
}

/** 持久化缩放值 */
function saveZoom(level: number): void {
  try {
    localStorage.setItem(ZOOM_STORAGE_KEY, level.toString());
  } catch {
    // localStorage 不可用时静默忽略
  }
}

/** 缩放值转百分比字符串 */
function toPercent(level: number): string {
  return `${Math.round(level * 100)}%`;
}

/**
 * 应用缩放到 DOM。
 *
 * transform: scale + 尺寸补偿：transform 使 #app 成为 fixed 子孙的 containing
 * block，布局尺寸 = 视口 / z，缩放后视觉恒精确填满窗口（zoom 方案在 z>1 裁切、
 * z<1 留白）。副作用：fixed inset-0 元素相对 #app 盒定位，视觉仍填满视口。
 */
function applyZoom(level: number): void {
  const app = document.getElementById("app");
  if (app) {
    app.style.transformOrigin = "top left";
    app.style.transform = `scale(${level})`;
    // #app 铺满整个动态视口（含安全区）；同 App.vue 的 position: fixed 口径：
    // 布局尺寸 = 视口 / z，缩放后视觉恒精确填满视口。
    app.style.width = `calc(100dvw / ${level})`;
    app.style.height = `calc(100dvh / ${level})`;
  }
  // 防御性归零页面级滚动：transform 方案下 #app 布局尺寸 = 视口 / z，超出视口
  // 使 body 成为可滚动容器，任何 scrollIntoView/编程滚动都可能把 fixed 面板
  // （containing block 是 #app）滚出视口。归零保证缩放后页面始终锚定原点。
  // （源头修复见 SettingsNav：tab 指示条滚动已限定在 nav 容器内。）
  window.scrollTo(0, 0);
  document.body.scrollLeft = 0;
  document.body.scrollTop = 0;
}

/** 已施加的 webview 页面缩放（只由 syncHostZoom 改写，用于累乘校正量） */
let hostZoom = 1;
/** 校正串行化：进出桌宠与 resize 会连着触发，排队执行避免中途插队改错缩放 */
let hostSyncQueue: Promise<void> = Promise.resolve();

/**
 * 量出 CSS 视口宽相对窗口逻辑宽的偏差。
 * 1 = 两者相等（正常）；小于 1 = 视口被压缩，按逻辑像素布局的界面会溢出窗口。
 */
async function measureViewportError(): Promise<number> {
  const win = getCurrentWindow();
  // 两次读取都要在 await 之后再取视口宽：窗口尺寸与视口得是同一时刻的值
  const [scaleFactor, size] = await Promise.all([win.scaleFactor(), win.innerSize()]);
  // 窗口尚未落地（尺寸为 0）时量不出偏差，按已对齐处理
  if (!(size.width > 0) || !(scaleFactor > 0)) return 1;
  return window.innerWidth / (size.width / scaleFactor);
}

function syncHostZoom(ownScaling: boolean): void {
  if (!isWindows()) return;
  hostSyncQueue = hostSyncQueue.then(async () => {
    try {
      // 文本大小只放大不缩小，偏差理应落在 1~0.44；越界的测量值（窗口未落地等）按已对齐处理
      const error = ownScaling ? await measureViewportError() : 1;
      const correction = error >= 0.4 && error <= 1.001 ? error : 1;
      const target = ownScaling ? hostZoom * correction : ZOOM_DEFAULT;
      // 偏差不足 0.5% 视为已对齐：省掉无谓 IPC，也切断「改缩放 → resize → 再校正」的回环
      if (Math.abs(target - hostZoom) < 0.005) return;
      await getCurrentWebview().setZoom(target);
      hostZoom = target;
    } catch {
      // 权限未授予或平台不支持：保持现状
    }
  });
}

/**
 * 激活 Ctrl+滚轮 UI 缩放功能。
 * 应在 App.vue 等始终挂载的根组件中调用一次。
 */
export function useZoom(): void {
  const route = useRoute();
  // 桌宠相关窗口自成一套缩放（--pet-ui-scale + 固定尺寸的无边框窗口），必须忽略主界面
  // 缩放：applyZoom 的尺寸补偿会把 #app 布局盒改成「视口 / z」，而窗口尺寸并没有跟着变，
  // 结果是布局坐标系与视口不一致 —— inset-x-0 的元素会比视口宽，右侧与底部被窗口裁掉。
  // 气泡窗（/bubble）同样属于桌宠，漏掉它就会出现「整个气泡窗被裁」。
  // TODO 小组件是另一个尺寸独立于主窗口的窗口，同理必须豁免。
  const ownScalingRoutes = new Set(["/pet", "/bubble", "/todo-widget"]);
  const ownScaling = () => ownScalingRoutes.has(route.path);
  const effectiveZoom = () => (ownScaling() ? ZOOM_DEFAULT : currentZoom.value);

  // 初始化 + 进出桌宠时都按当前路由重新应用（桌宠/气泡窗里恒为 100%）
  watch(
    () => route.path,
    () => {
      applyZoom(effectiveZoom());
      // 同一窗口会在主界面与桌宠之间切路由，页面缩放也要跟着切
      syncHostZoom(ownScaling());
    },
    { immediate: true },
  );

  // 文本大小改了、桌宠窗尺寸变了都会触发 resize，重量一次偏差（已对齐时是空操作）。
  // 只认桌宠族路由：主窗口可自由拖拽，拖一次会刷出几十个 resize，别再叠测量 IPC。
  const handleResize = () => {
    if (ownScaling()) syncHostZoom(true);
  };
  window.addEventListener("resize", handleResize);

  const handleWheel = (event: WheelEvent) => {
    if (!event.ctrlKey) return;

    // 桌宠相关窗口：吞掉 Ctrl+滚轮（既不缩放界面，也不让它触发 WebView 原生缩放）
    if (ownScalingRoutes.has(route.path)) {
      event.preventDefault();
      return;
    }

    // 阻止浏览器默认的缩放行为
    event.preventDefault();

    // 向下滚动（deltaY > 0）= 缩小，向上滚动（deltaY < 0）= 放大
    const delta = event.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
    const newZoom = Math.round((currentZoom.value + delta) * 100) / 100;
    const clamped = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, newZoom));

    // 四舍五入到指定小数位数
    currentZoom.value =
      Math.round(clamped * Math.pow(10, ZOOM_DECIMALS)) / Math.pow(10, ZOOM_DECIMALS);

    applyZoom(currentZoom.value);
    saveZoom(currentZoom.value);

    // 防抖显示缩放百分比 toast
    const now = Date.now();
    if (now - lastToastTime > TOAST_DEBOUNCE_MS) {
      lastToastTime = now;
      const uiStore = useUIStore();
      uiStore.showInfo({
        title: i18n.global.t("stores.zoom.toastTitle"),
        message: toPercent(currentZoom.value),
        duration: 800,
      });
    }
  };

  // 使用 passive: false 以允许 preventDefault 阻止浏览器默认缩放
  window.addEventListener("wheel", handleWheel, { passive: false });

  onUnmounted(() => {
    window.removeEventListener("wheel", handleWheel);
    window.removeEventListener("resize", handleResize);
  });
}

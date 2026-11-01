import { createApp } from "vue";
import pinia from "./stores";
import { initializeEventProcessors } from "./core/events";
import { initializeTauriEventListeners, initializeCastWindowListeners } from "./api/tauri-events";

import App from "./App.vue";
import mainMenuCharacterUrl from "./assets/images/alona.webp";
import "./assets/styles/base.css";
import "./assets/styles/variables.css";
import "./assets/styles/dialogue-text.css";
import { i18n, localeReady } from "./locales";

// WebSocket handlers 保留用于未来剧本模式参考
// import "./api/websocket/handlers/script-handler";
// import "./api/websocket/handlers/adventure-handler";

import { getCurrentWindow } from "@tauri-apps/api/window";
import router from "./router";
import { installRouteFade } from "./composables/app/useRouteFade";
import { autoConfigurePerformance } from "./api/services/cpu-perf";
import { initAudioOutputManager } from "./utils/audioOutputManager";

// 仅主窗口启动时清除加载过渡标记，避免设置窗口等其他窗口误清除
if (getCurrentWindow().label === "main") {
  localStorage.removeItem("lingchat_loading_shown");
}

const app = createApp(App);

initializeEventProcessors();

// 投屏窗口是独立 webview：不注册驱动事件队列的全局监听（ai:reply 等），
// 台词由主窗口镜像（cast:mirror）驱动，只注册投屏需要的即时状态事件。
// 气泡窗与 TODO 小组件同样不参与事件队列，各自只挂自己需要的事件。
const windowKind = new URLSearchParams(window.location.search).get("window");
const isCastWindow = windowKind === "cast";
const isBubbleWindow = windowKind === "bubble";
const isTodoWidgetWindow = windowKind === "todo_widget";

// 主菜单立绘是首屏 LCP 元素。生产构建由 vite 插件在 index.html 静态注入 preload；
// dev 下没有这一步，这里补一个运行时预加载。独立窗口不渲染主菜单，跳过。
if (import.meta.env.DEV && !windowKind) {
  const preload = document.createElement("link");
  preload.rel = "preload";
  preload.as = "image";
  preload.href = mainMenuCharacterUrl;
  preload.setAttribute("fetchpriority", "high");
  document.head.appendChild(preload);
}

if (isCastWindow) {
  initializeCastWindowListeners();
} else if (!isBubbleWindow && !isTodoWidgetWindow) {
  initializeTauriEventListeners();
}

app.use(pinia);
app.use(i18n);

// 主菜单是首屏路由：尽早预取组件 chunk，避免 LCP 等待首次导航串行加载
if (!windowKind) {
  void import("./components/views/MainMenu.vue");
}

// 路由黑屏过渡守卫：必须早于 app.use(router) 触发的首次导航，且只注册一次
installRouteFade(router);
app.use(router);

// 独立日志窗口：通过 index.html?window=log 打开时直接进入日志路由
if (windowKind === "log") {
  router.replace("/log-window");
}

// 投屏窗口：通过 index.html?window=cast 打开时直接进入投屏路由
if (isCastWindow) {
  router.replace("/cast");
}

// 气泡窗口：桌宠模式下由宠物窗创建，只画气泡，不参与事件队列
if (isBubbleWindow) {
  router.replace("/bubble");
}

// TODO 小组件：由主窗口创建，只画倒计时与清单，不参与事件队列
if (isTodoWidgetWindow) {
  router.replace("/todo-widget");
}

// 语言包按需加载：非中文界面先载入对应词条再挂载，避免首屏闪现中文。
// 默认的中文词条已同步就绪，这里通常只等待一次微任务。
void localeReady.finally(() => {
  app.mount("#app");

  // 初始化全局音频输出设备管理器（需 pinia 就绪）
  initAudioOutputManager();

  // 延迟执行 CPU+GPU 画质自适应，确保 pinia store 已就绪
  setTimeout(autoConfigurePerformance, 1000);
});

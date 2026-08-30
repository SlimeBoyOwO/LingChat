import { createApp } from "vue";
import pinia from "./stores";
import { initializeEventProcessors } from "./core/events";
import { initializeTauriEventListeners, initializeCastWindowListeners } from "./api/tauri-events";

import App from "./App.vue";
import "./assets/styles/base.css";
import "./assets/styles/variables.css";
import { i18n } from "./locales";

// WebSocket handlers 保留用于未来剧本模式参考
// import "./api/websocket/handlers/script-handler";
// import "./api/websocket/handlers/adventure-handler";

import { getCurrentWindow } from "@tauri-apps/api/window";
import router from "./router";
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
const isCastWindow = new URLSearchParams(window.location.search).get("window") === "cast";
if (isCastWindow) {
  initializeCastWindowListeners();
} else {
  initializeTauriEventListeners();
}

app.use(pinia);
app.use(i18n);
app.use(router);

// 独立日志窗口：通过 index.html?window=log 打开时直接进入日志路由
if (new URLSearchParams(window.location.search).get("window") === "log") {
  router.replace("/log-window");
}

// 投屏窗口：通过 index.html?window=cast 打开时直接进入投屏路由
if (isCastWindow) {
  router.replace("/cast");
}

app.mount("#app");

// 初始化全局音频输出设备管理器（需 pinia 就绪）
initAudioOutputManager();

// 延迟执行 CPU+GPU 画质自适应，确保 pinia store 已就绪
setTimeout(autoConfigurePerformance, 1000);

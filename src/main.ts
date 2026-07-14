import { createApp } from "vue";
import pinia from "./stores";
import { initializeEventProcessors } from "./core/events";
import { initializeTauriEventListeners } from "./api/tauri-events";

import App from "./App.vue";
import "./assets/styles/base.css";
import "./assets/styles/variables.css";

// WebSocket handlers 保留用于未来剧本模式参考
// import "./api/websocket/handlers/script-handler";
// import "./api/websocket/handlers/adventure-handler";

import router from "./router";
import { autoConfigureCpuPerformance } from "./api/services/cpu-perf";

// 每次启动时清除加载过渡标记，确保首次进入 MainChat 时播放加载动画
localStorage.removeItem('lingchat_loading_shown')

const app = createApp(App);

initializeEventProcessors();
initializeTauriEventListeners();

app.use(pinia);
app.use(router);
app.mount("#app");

// 延迟执行 CPU 画质自适应，确保 pinia store 已就绪
setTimeout(autoConfigureCpuPerformance, 1000);

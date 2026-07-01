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
import { getCpuInfo, getSuggestedMaxFps, getSuggestedParticleScale } from "./api/services/cpu-perf";

const app = createApp(App);

initializeEventProcessors();
initializeTauriEventListeners();

app.use(pinia);
app.use(router);
app.mount("#app");

/**
 * 根据 CPU 性能等级自动调整画质设定
 * 仅首次启动时生效（后端已做缓存），不覆盖用户手动调整的值。
 */
async function autoConfigurePerformance(): Promise<void> {
  try {
    const info = await getCpuInfo();
    const fps = getSuggestedMaxFps(info.tier);
    const particleScale = getSuggestedParticleScale(info.tier);

    // 从 pinia 获取 settingsStore 并应用推荐值
    const { useSettingsStore } = await import("./stores/modules/settings");
    const settingsStore = useSettingsStore();

    // 仅当当前值等于默认值时覆盖（避免覆盖用户手动调整）
    const { DEFAULT_SETTINGS } = await import("./stores/modules/settings");

    if (settingsStore.display.meteorFps === DEFAULT_SETTINGS.display.meteorFps) {
      settingsStore.setMeteorFps(Math.min(fps, 60));
    }
    if (settingsStore.display.starsFps === DEFAULT_SETTINGS.display.starsFps) {
      settingsStore.setStarsFps(Math.min(fps, 60));
    }

    console.log(
      `[CPU-Perf] ${info.brand} → ${info.tier}, 建议帧率 ${fps}FPS, 粒子比例 ${particleScale}`,
    );
  } catch (e) {
    // 静默失败：不影响主流程
    console.warn("[CPU-Perf] 自动配置失效，使用默认画质", e);
  }
}

// 延迟执行，确保 pinia store 已就绪
setTimeout(autoConfigurePerformance, 1000);

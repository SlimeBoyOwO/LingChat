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
import { getCpuInfo, getSuggestedMaxFps, getSuggestedParticleScale, getRecommendedEffects } from "./api/services/cpu-perf";

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
    const { useSettingsStore, DEFAULT_SETTINGS } = await import("./stores/modules/settings");
    const settingsStore = useSettingsStore();

    if (settingsStore.display.meteorFps === DEFAULT_SETTINGS.display.meteorFps) {
      settingsStore.setMeteorFps(Math.min(fps, 60));
    }
    if (settingsStore.display.starsFps === DEFAULT_SETTINGS.display.starsFps) {
      settingsStore.setStarsFps(Math.min(fps, 60));
    }

    // 低性能设备自动关闭高开销特效（仅当用户未手动修改时）
    if (info.tier === 'Internet' || info.tier === 'Low') {
      const effects = getRecommendedEffects(info.tier);
      if (settingsStore.display.mainMenuStarsEnabled === DEFAULT_SETTINGS.display.mainMenuStarsEnabled) {
        settingsStore.setMainMenuStarsEnabled(effects.mainMenuStarsEnabled);
      }
      if (settingsStore.display.mainMenuMeteorsEnabled === DEFAULT_SETTINGS.display.mainMenuMeteorsEnabled) {
        settingsStore.setMainMenuMeteorsEnabled(effects.mainMenuMeteorsEnabled);
      }
      if (settingsStore.display.globalMouseTrailEnabled === DEFAULT_SETTINGS.display.globalMouseTrailEnabled) {
        settingsStore.setGlobalMouseTrailEnabled(effects.globalMouseTrailEnabled);
      }
      if (settingsStore.display.clickAnimationEnabled === DEFAULT_SETTINGS.display.clickAnimationEnabled) {
        settingsStore.setClickAnimationEnabled(effects.clickAnimationEnabled);
      }
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

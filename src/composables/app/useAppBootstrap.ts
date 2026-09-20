/**
 * 应用启动初始化 Composable
 *
 * 集中处理挂载时的一次性初始化：
 *   - UI Store（加载角色 tips）
 *   - 自动弹出独立日志窗口（仅主窗口触发，开关在日志页设置）
 *   - LLM 提供商配置预加载，避免主界面因 store 未加载而误判未选择模型
 *   - 光影预设预取与生效状态上报（仅主窗口）
 *   - 成就系统的 window 控制台测试钩子 + WebSocket 解锁监听
 *
 * 在 App.vue 中调用一次。
 */
import { onMounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { initUIStore } from "@/stores/modules/ui/ui";
import { useLlmProvidersStore } from "@/stores/modules/llm-providers";
import { useLightingStore } from "@/stores/modules/lighting";
import { useAchievementStore } from "@/stores/modules/ui/achievement";

export function useAppBootstrap() {
  onMounted(() => {
    const isMainWindow = getCurrentWindow().label === "main";

    // 初始化 UI Store（加载角色 tips）
    initUIStore();

    // 启动时自动弹出独立日志窗口（仅主窗口触发，开关在日志页设置）
    if (isMainWindow && localStorage.getItem("lingchat_log_window_auto_open") === "1") {
      invoke("open_log_window").catch((e) => console.error("自动打开日志窗口失败:", e));
    }

    // 预加载 LLM 提供商配置，避免主界面因 store 未加载而误判未选择模型
    const llmStore = useLlmProvidersStore();
    llmStore.load().catch((e) => console.error("加载 LLM 提供商失败:", e));

    // 光影预设要在渲染前拿到，「默认光影」才解析得出名字；生效状态只由主窗口上报，
    // 投屏一起报会让两边互相覆盖后端那份状态。这两件放在这里而不是 tauri-events：
    // initializeTauriEventListeners 跑在 app.use(pinia) 之前，提前取 store 会白屏。
    if (isMainWindow) {
      const lightingStore = useLightingStore();
      void lightingStore.ensurePresets();
      lightingStore.startActiveReporting();
    }

    // 供成就系统控制台测试用，在 window 对象中注册一些方法
    const achievementStore = useAchievementStore();
    (window as any).requestAchievementUnlock = (data: any) =>
      achievementStore.notifyBackendUnlock(data);
    (window as any).showAchievement = (data: any) => achievementStore.addAchievement(data);
    // 成就系统启动WebSocket监听
    achievementStore.listenForUnlocks();
  });
}

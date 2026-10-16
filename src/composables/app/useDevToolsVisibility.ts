/**
 * Vue DevTools 面板显隐 Composable（仅开发模式）
 *
 * vite-plugin-vue-devtools 只在 dev serve 注入悬浮面板（apply: "serve"），
 * 面板挂载到 body 下的 #__vue-devtools-container__；组件高亮层是另一个
 * #__vue-devtools-component-inspector__ 容器。面板只跟随设置显示在主窗口；
 * 日志/投屏/桌宠等独立窗口永远隐藏（白底 pill 在暗色日志窗口里是底部白条，
 * 投屏会把它抓进串流画面）。App.vue 是所有窗口的根，这里生效即「全局隐藏」。
 *
 * 在 App.vue 中调用一次以激活该开关。
 */
import { watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettingsStore } from "@/stores/modules/settings";

const DEVTOOLS_CONTAINER_IDS = [
  "__vue-devtools-container__",
  "__vue-devtools-component-inspector__",
];

export function useDevToolsVisibility() {
  const settingsStore = useSettingsStore();
  // 悬浮面板（light 主题白底 pill）只在主窗口跟随设置显示：日志/投屏/桌宠等
  // 独立窗口永远隐藏——日志窗口暗色主题下它是底部一条刺眼的白色圆角条，
  // 投屏窗口则会把它抓进串流画面。
  const isMainWindow = getCurrentWindow().label === "main";

  function applyVueDevToolsVisibility() {
    if (!import.meta.env.DEV) return;
    const visible = isMainWindow && settingsStore.text.vueDevToolsEnabled;
    for (const id of DEVTOOLS_CONTAINER_IDS) {
      const el = document.getElementById(id);
      if (el) el.style.display = visible ? "" : "none";
    }
  }

  watch(() => settingsStore.text.vueDevToolsEnabled, applyVueDevToolsVisibility, {
    immediate: true,
  });
  // 面板由 overlay 脚本动态 import 后挂载，可能晚于 App.vue 初始化（尤其
  // 关闭状态需在容器出现时就隐藏）。监听 body 子树，容器出现即补一次。
  if (import.meta.env.DEV) {
    const devtoolsObserver = new MutationObserver(() => applyVueDevToolsVisibility());
    devtoolsObserver.observe(document.body, { childList: true, subtree: true });
  }
}

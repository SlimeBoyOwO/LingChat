/**
 * F11 全屏快捷键 Composable
 *
 * 注册全局 keydown 监听并切换窗口全屏。桌宠路由（/pet）下不生效：
 * 该模式是无边框小窗，全屏没有意义。
 *
 * 在 App.vue 中调用一次以激活。
 */
import { onMounted, onUnmounted } from "vue";
import { useRoute } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function useFullscreenHotkey() {
  const route = useRoute();

  const handleKeyDown = async (event: KeyboardEvent) => {
    if (event.key !== "F11") return;
    event.preventDefault();

    // Pet 路由时不允许全屏
    if (route.path === "/pet") {
      return;
    }

    try {
      const appWindow = getCurrentWindow();
      const isFullscreen = await appWindow.isFullscreen();
      await appWindow.setFullscreen(!isFullscreen);
    } catch (e) {
      console.error("全屏切换失败:", e);
    }
  };

  onMounted(() => window.addEventListener("keydown", handleKeyDown));
  onUnmounted(() => window.removeEventListener("keydown", handleKeyDown));
}

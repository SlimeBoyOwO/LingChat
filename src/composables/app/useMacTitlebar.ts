/**
 * macOS 无边框标题栏（Overlay）顶部安全区 Composable
 *
 * 主窗口使用 titleBarStyle: Overlay（保留红绿灯、去掉标题栏）后，内容会顶到窗口
 * 上边缘，与 macOS 原生红绿灯/系统标题栏重叠。这里把「顶部安全区」变量抬升为
 * 一条固定的标题栏高度，让所有使用 var(--safe-area-inset-top) 的顶部组件统一让位，
 * 避免贴近窗口边缘/被红绿灯遮挡。仅 macOS 主窗口（且非桌宠模式）生效：
 * 桌宠模式会调用 set_decorations(false) 变成无边框小窗，此时红绿灯隐藏、不需要让位；
 * 其它平台与窗口保持原值。
 *
 * 返回 isMacOverlayWindow 供根模板决定是否渲染顶部拖拽区。
 * 在 App.vue 中调用一次。
 */
import { computed, watch } from "vue";
import { useRoute } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isMacOS } from "@/utils/platform";

const MAC_TITLEBAR_INSET_PX = 40;

export function useMacTitlebar() {
  const route = useRoute();
  const isMainWindow = getCurrentWindow().label === "main";
  const isMac = isMacOS();
  // 桌宠模式复用主窗口（./pet），此时窗口为无边框小窗，顶部无需让位也无标题栏拖拽区
  const isMacOverlayWindow = computed(() => isMac && isMainWindow && route.path !== "/pet");

  function applyMacTitlebarInset() {
    if (isMacOverlayWindow.value) {
      document.documentElement.style.setProperty(
        "--safe-area-inset-top",
        `${MAC_TITLEBAR_INSET_PX}px`,
      );
    } else {
      document.documentElement.style.removeProperty("--safe-area-inset-top");
    }
  }
  applyMacTitlebarInset();
  watch(isMacOverlayWindow, applyMacTitlebarInset);

  return { isMacOverlayWindow };
}

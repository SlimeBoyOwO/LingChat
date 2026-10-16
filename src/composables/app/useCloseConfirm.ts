/**
 * 关闭确认 Composable
 *
 * 退出需要同时满足两个条件：Rust 侧存档完成（app:close-ready）+ 用户确认，
 * 二者齐备才调用 exit_app。仅主窗口需要弹确认框，其它窗口（投屏/日志）
 * 正常关闭。
 *
 * 在 App.vue 中调用一次。
 */
import { onMounted, onUnmounted } from "vue";
import { useRoute } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { i18n } from "@/locales";
import { useDialogStore } from "@/stores/modules/ui/dialog";

export function useCloseConfirm() {
  const route = useRoute();
  const dialogStore = useDialogStore();
  let saveCompleted = false;
  let userConfirmedExit = false;
  let unlistenCloseReady: (() => void) | null = null;
  let unlistenCloseRequested: (() => void) | null = null;

  // 处理退出：两个条件都满足时调用 Rust exit_app
  function tryExit() {
    if (saveCompleted && userConfirmedExit) {
      invoke("exit_app");
    }
  }

  onMounted(async () => {
    // 1. 监听 Rust 存档完成事件
    unlistenCloseReady = await listen("app:close-ready", () => {
      saveCompleted = true;
      tryExit();
    });

    // 2. 拦截窗口关闭请求（仅主窗口需要确认，其他窗口正常关闭）
    unlistenCloseRequested = await getCurrentWindow().onCloseRequested(
      async (event: { preventDefault: () => void }) => {
        if (getCurrentWindow().label !== "main") return;

        event.preventDefault();

        // 重置状态
        saveCompleted = false;
        userConfirmedExit = false;

        if (route.path === "/chat") {
          const confirmed = await dialogStore.confirm(
            i18n.global.t("common.exitMessage"),
            i18n.global.t("common.exitTitle"),
          );
          if (!confirmed) return; // 用户取消，窗口保持打开
        }

        userConfirmedExit = true;
        tryExit();
      },
    );
  });

  onUnmounted(() => {
    if (unlistenCloseReady) unlistenCloseReady();
    if (unlistenCloseRequested) unlistenCloseRequested();
  });
}

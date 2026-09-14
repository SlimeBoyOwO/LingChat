import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * 截图状态 —— 主界面 GameDialog 与桌宠（GameRolesStage / ChatInput）共用。
 *
 * 状态是模块级单例：多个组件同时挂载时靠 init/destroy 的引用计数只注册一份监听。
 * （主界面与桌宠是不同路由、不会同时挂载，计数只是给同路由内多组件用。）
 */
const hasScreenshot = ref(false);
const screenshotBase64 = ref<string | null>(null);
const isCapturing = ref(false);
let unlisten: (() => void) | null = null;
let unlistenCanceled: (() => void) | null = null;
let initCount = 0;
let disposed = false;

export interface StartScreenshotOptions {
  /**
   * 启动截图失败时的回调。各调用方的提示方式不同（主界面弹 alert，桌宠静默），
   * 所以由调用方提供，而不是写在共享件里。
   */
  onError?: (error: unknown) => void;
}

export function useScreenshot() {
  function init() {
    if (initCount++ > 0) return;
    disposed = false;
    listen<{ base64: string }>("screenshot:captured", (event) => {
      screenshotBase64.value = event.payload.base64;
      hasScreenshot.value = true;
      isCapturing.value = false;
    })
      .then((fn) => {
        // listen resolve 前组件已 destroy：立即反注册，避免残留监听
        if (disposed) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch((e) => console.error("监听截图完成事件失败:", e));

    listen("screenshot:cancelled", () => {
      //监听截图取消事件
      isCapturing.value = false;
      hasScreenshot.value = false;
      // 必须一并清掉 base64：否则「截图 → 取消 → 发送」会把上一次截的图带上。
      // 清 hasScreenshot 是不够的 —— clear() 在 hasScreenshot 为 false 时会直接 return。
      screenshotBase64.value = null;
    })
      .then((fn) => {
        if (disposed) {
          fn();
          return;
        }
        unlistenCanceled = fn;
      })
      .catch((e) => console.error("监听截图取消事件失败:", e));
  }

  function destroy() {
    if (--initCount > 0) return;
    disposed = true;
    if (unlisten) {
      unlisten();
      unlisten = null;
    }

    if (unlistenCanceled) {
      unlistenCanceled();
      unlistenCanceled = null;
    }
  }

  async function start(options?: StartScreenshotOptions) {
    if (isCapturing.value) return;
    isCapturing.value = true;
    try {
      await invoke("start_screenshot");
    } catch (error) {
      console.error("启动截图失败:", error);
      isCapturing.value = false;
      options?.onError?.(error);
    }
  }

  function clear() {
    if (hasScreenshot.value) {
      hasScreenshot.value = false;
      screenshotBase64.value = null;
    }
  }

  return {
    hasScreenshot,
    screenshotBase64,
    isCapturing,
    init,
    destroy,
    start,
    clear,
  };
}

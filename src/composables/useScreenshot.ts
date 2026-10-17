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
    listen<{ base64: string }>("screenshot:captured", (event) => {
      screenshotBase64.value = event.payload.base64;
      hasScreenshot.value = true;
      isCapturing.value = false;
    }).then((fn) => {
      unlisten = fn;
    });

    listen("screenshot:cancelled", () => {
      //监听截图取消事件
      isCapturing.value = false;
      hasScreenshot.value = false;
      // 必须一并清掉 base64：否则「截图 → 取消 → 发送」会把上一次截的图带上。
      // 清 hasScreenshot 是不够的 —— clear() 在 hasScreenshot 为 false 时会直接 return。
      screenshotBase64.value = null;
    }).then((fn) => {
      unlistenCanceled = fn;
    });
  }

  function destroy() {
    if (--initCount > 0) return;
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

  /**
   * 直接抓一帧当前屏幕，**不框选、不走拍照/相册**。
   *
   * ## 谁用这条
   *
   * Android 桌宠模式。桌面端与聊天页仍然走 {@link start}：
   *
   * - 桌面元素杂，框选有意义 → 保留 `screenshot-overlay` 那条链路
   * - 手机一屏就那么点内容，框选纯属多一步 → 整屏送进去反而更准
   * - 桌宠态下我们的 WebView 被搬在 `TYPE_APPLICATION_OVERLAY` 悬浮窗里，
   *   也开不出全屏覆盖窗口来框选
   *
   * ## 与 {@link start} 的关键差别
   *
   * `start` 只是「发起」，真正的图由 `screenshot:captured` 事件异步送回来；
   * 本函数**直接拿到 base64**，不依赖任何事件 —— 所以也不会有
   * `isCapturing` 卡死的风险（`finally` 一定复位）。
   */
  async function capture(options?: StartScreenshotOptions) {
    if (isCapturing.value) return;
    isCapturing.value = true;
    try {
      const base64 = await invoke<string>("take_screenshot_base64");
      if (!base64) throw new Error("截到的是空图");
      screenshotBase64.value = base64;
      hasScreenshot.value = true;
    } catch (error) {
      console.error("直接截图失败:", error);
      options?.onError?.(error);
    } finally {
      // 必须无条件复位：这里没有「等事件回来」的异步尾巴，
      // 失败时更要复位，否则按钮会永久失效（`start` 踩过这个坑）。
      isCapturing.value = false;
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
    capture,
    clear,
  };
}

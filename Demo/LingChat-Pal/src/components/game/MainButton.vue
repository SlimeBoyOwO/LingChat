<template>
  <button class="open-window-btn" @click="openSecondWindow">
    {{ buttonText }}
  </button>
</template>

<script setup lang="ts">
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

// 定义组件属性
defineProps({
  buttonText: {
    type: String,
    default: "诺一 钦灵",
  },
});

// 定义事件
const emit = defineEmits(["click"]);

// 打开第二个窗口的函数
const openSecondWindow = async () => {
  // 触发点击事件，让父组件处理
  emit("click");

  try {
    // 根据运行环境选择正确的 URL：
    // - 开发模式 (Vite dev server)：使用当前 location.origin + #/second（例如 http://localhost:5173/#/second）
    // - 打包到 Tauri：使用 index.html#/second（Tauri 会加载 dist 的 index.html）
    const isDev = Boolean((import.meta as any).env?.DEV);
    const targetUrl = isDev
      ? `${window.location.origin}#/second`
      : "index.html#/second";

    const webview = new WebviewWindow("settings", {
      url: targetUrl,
      title: "设置",
      width: 1200,
      height: 800,
      resizable: true,
      shadow: false,
      decorations: false,
      transparent: true,
      alwaysOnTop: false,
      visible: true,
    });

    webview.once("tauri://created", () => {
      console.log("第二个窗口创建成功");
    });

    webview.once("tauri://error", (e) => {
      console.error("创建第二个窗口失败:", e);
    });
  } catch (error) {
    console.error("打开第二个窗口时出错:", error);
  }
};
</script>

<style scoped>
.open-window-btn {
  position: relative;
  top: -20px;
  left: 50%;
  transform: translateX(-50%);
  padding: 8px 16px;
  background: rgba(50, 158, 163, 0.8);
  color: white;
  border: none;
  border-radius: 15px;
  cursor: pointer;
  z-index: 10;
}

.open-window-btn:hover {
  background: rgba(50, 158, 163, 1);
}
</style>

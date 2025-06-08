import { ChatSocket } from "./core/connection.js"; // 路径看你实际放哪
import { DOM } from "./ui/dom.js";
import { ChatManager } from "./features/chat/manager.js";
import { UIController } from "./ui/ui-controller.js";
import { StarField } from "./features/background/star-field.js";
import { HistoryManager } from "./features/history/manager.js";
import { SoundController } from "./features/sound/controller.js";
import { MenuController } from "./features/menu/controller.js";
import { ImageController } from "./features/image/controller.js";
import { SaveController } from "./features/save/controller.js";

// 初始化模块
const protocol = window.location.protocol === "https:" ? "wss" : "ws";
const host = window.location.hostname;
// 使用当前页面的主机和端口，确保WebSocket连接到正确的服务器
const port = window.location.port || (protocol === "wss" ? "443" : "80");
const wsUrl = `${protocol}://${host}:${port}/ws`;
const socket = new ChatSocket(wsUrl);

const uiController = new UIController();
const historyManager = new HistoryManager();
const chatManager = new ChatManager({
  connection: socket,
  historyManager: historyManager,
});
const soundController = new SoundController();
const menuController = new MenuController(uiController);
const imageController = new ImageController();
const saveController = new SaveController();

// 多模块组合处理函数

// 初始化应用
document.addEventListener("DOMContentLoaded", () => {
  const canvas = DOM.canvas;

  if (canvas) {
    const starField = new StarField(canvas);

    // 切换页面时销毁
    window.addEventListener("beforeunload", () => starField.destroy());
  }
});

// 页面加载完成后执行的函数
window.addEventListener("load", function () {
  const loader = document.getElementById("loader");
  const mainContent = document.getElementById("main-content");

  // 模拟一个最小加载时间，防止动画一闪而过
  setTimeout(() => {
    // 隐藏加载动画
    loader.classList.add("hidden");

    // 显示主内容
    mainContent.classList.add("visible");
  }, 1500); // 你可以根据需要调整这里的延迟时间（单位：毫秒）
});

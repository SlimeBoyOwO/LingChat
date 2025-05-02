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
const host = window.location.hostname || "localhost";
const protocol = window.location.protocol === "https:" ? "wss" : "ws";
const wsUrl = `${protocol}://${host}:3000/ws`;
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

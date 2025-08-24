import { ChatSocket } from "../core/connection"; // 路径看你实际放哪
import { DOM } from "../ui/dom";
import { UIController } from "../ui/ui-controller";
import { MenuController } from "../features/menu/controller";
import { TextController } from "../features/menu/text/controller";
import { SaveController } from "../features/menu/save/controller";
import { SoundController } from "../features/menu/sound/controller";
import { ImageController } from "../features/menu/image/controller";
import { CharacterController } from "../features/menu/character/controller";
import { StarField } from "../features/particles/star_field";

import { ChatManager } from "../features/chat/manager";
import { HistoryController } from "../features/menu/history/controller";

export async function initApp(): Promise<void> {
  // 初始化模块
  const protocol: string = window.location.protocol === "https:" ? "wss" : "ws";
  const host: string = window.location.hostname;
  // 使用当前页面的主机和端口，确保WebSocket连接到正确的服务器
  const port: string =
    window.location.port || (protocol === "wss" ? "443" : "80");
  const wsUrl: string = `${protocol}://${host}:${port}/ws`;
  const socket: ChatSocket = new ChatSocket(wsUrl);
  const uiController: UIController = new UIController();
  const menuController: MenuController = new MenuController();
  const textController: TextController = new TextController();
  const saveController: SaveController = new SaveController();
  const soundController: SoundController = new SoundController();
  const imageController: ImageController = new ImageController();
  const historyController: HistoryController = new HistoryController();
  const characterController: CharacterController = new CharacterController();
  const chatManager: ChatManager = new ChatManager({
    socket,
    historyController,
  });

  // 初始化应用━△
  document.addEventListener("DOMContentLoaded", (): void => {
    const canvas: HTMLCanvasElement | null = DOM.canvas;

    if (canvas) {
      const starField: StarField = new StarField(canvas);

      // 切换页面时销毁
      window.addEventListener("beforeunload", (): void => {
        starField.destroy();
        console.log("星空背景已经被销毁");
      });
    } else {
      console.log("心控背景加载错误哦");
    }
  });

  // 页面加载完成后执行的函数
  window.addEventListener("load", function (): void {
    const loader: HTMLElement | null = document.getElementById("loader");

    if (loader) {
      // 模拟一个最小加载时间，防止动画一闪而过
      setTimeout((): void => {
        // 隐藏加载动画
        loader.classList.add("hidden");
      }, 1500); // 你可以根据需要调整这里的延迟时间（单位：毫秒）
    }
  });
}

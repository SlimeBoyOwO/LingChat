import { DOM } from "../../ui/dom.js";
import { DomUtils } from "../../utils/dom-utils.js";
import { ConversationLoader } from "./conversation-loader.js";

export class SaveController {
  constructor() {
    this.processing = false;
    this.domUtils = DomUtils;
    this.conversationLoader = new ConversationLoader("conversation-list", 1, {
      // 可选配置项
      pageSize: 10,
    });
    this.init();
  }

  init() {
    this.bindEvents();
    this.setupLoaderCallbacks();
  }

  setupLoaderCallbacks() {
    // 覆盖 ConversationLoader 的默认回调方法
    this.conversationLoader.onConversationLoaded = (messages) => {
      console.log("对话内容加载完成:", messages);
      // 在这里处理加载的对话内容
      // 例如更新UI或触发其他操作
    };

    this.conversationLoader.onLoadError = (error) => {
      console.error("加载对话出错:", error);
    };
  }

  bindEvents() {
    if (!DOM.menuSave) return;

    DOM.menuSave.addEventListener("click", () => this.toggleSavePanel());
  }

  async toggleSavePanel() {
    if (this.processing) return;
    this.processing = true;

    requestAnimationFrame(async () => {
      // 显示存档相关元素
      this.domUtils.showElements([DOM.menuContent, DOM.menuSave, DOM.savePage]);

      // 隐藏其他面板元素
      this.domUtils.hideElements(
        this.domUtils.getOtherPanelElements([
          DOM.menuContent,
          DOM.menuSave,
          DOM.savePage,
        ])
      );

      await this.conversationLoader.loadConversations();

      setTimeout(() => {
        this.processing = false;
      }, 300);
    });
  }
}

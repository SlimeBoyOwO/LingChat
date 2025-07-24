import { DOM } from "../../../ui/dom";
import { DomUtils } from "../../../utils/dom-utils";
import { ConversationLoader } from "./conversation-loader";
import EventBus from "../../../core/event-bus";

export class SaveController {
  private processing: boolean;
  private domUtils: typeof DomUtils;
  private conversationLoader: ConversationLoader;

  constructor() {
    this.processing = false;
    this.domUtils = DomUtils;
    this.conversationLoader = new ConversationLoader("conversation-list", "1", {
      // 可选配置项
      pageSize: 10,
    });
    this.init();
  }

  private init() {
    this.bindEvents();
    this.setupLoaderCallbacks();
  }

  private setupLoaderCallbacks() {
    // 覆盖 ConversationLoader 的默认回调方法
    this.conversationLoader.onConversationLoaded = (messages) => {
      console.log("对话内容加载完成:", messages);
      EventBus.emit("system:character_updated", {});
    };

    this.conversationLoader.onLoadError = (error) => {
      console.error("加载对话出错:", error);
    };
  }

  private bindEvents() {
    if (!DOM.menuSave) return;

    DOM.menuSave.addEventListener("click", () => this.toggleSavePanel());
  }

  private async toggleSavePanel() {
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

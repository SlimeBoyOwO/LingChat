import { DOM } from "../../ui/dom.js";
import { DomUtils } from "../../utils/dom-utils.js";

export class SaveController {
  constructor() {
    this.processing = false;
    this.domUtils = DomUtils;
    this.init();
  }

  init() {
    this.bindEvents();
  }

  bindEvents() {
    if (!DOM.menuSave) return;

    DOM.menuSave.addEventListener("click", () => this.toggleSavePanel());
  }

  toggleSavePanel() {
    if (this.processing) return;
    this.processing = true;

    requestAnimationFrame(() => {
      // 显示存档相关元素
      this.domUtils.showElements([DOM.menuContent, DOM.menuSave, DOM.savePage]);

      // 隐藏其他面板元素
      this.domUtils.hideElements([
        DOM.menuImage,
        DOM.imagePage,
        DOM.history.toggle,
        DOM.history.content,
        DOM.history.clearBtn,
        DOM.menuText,
        DOM.textPage,
        DOM.soundPage,
      ]);

      setTimeout(() => {
        this.processing = false;
      }, 300);
    });
  }
}

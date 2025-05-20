import { DOM } from "../ui/dom.js";

export const DomUtils = {
  // 所有可切换的面板元素
  ALL_PANEL_ELEMENTS: [
    DOM.history.toggle,
    DOM.history.content,
    DOM.history.clearBtn,
    DOM.menuText,
    DOM.menuSave,
    DOM.textPage,
    DOM.menuImage,
    DOM.imagePage,
    DOM.menuSound,
    DOM.soundPage,
    DOM.savePage,
  ],

  /**
   * 显示多个元素
   * @param {Array<HTMLElement>} elements
   */
  showElements(elements) {
    elements.forEach((el) => el?.classList.add("show"));
  },

  /**
   * 隐藏多个元素
   * @param {Array<HTMLElement>} elements
   */
  hideElements(elements) {
    elements.forEach((el) => el?.classList.remove("show"));
  },

  /**
   * 切换元素显示状态
   * @param {HTMLElement} element
   * @param {Boolean} show
   */
  toggleElement(element, show) {
    element?.classList.toggle("show", show);
  },

  /**
   * 获取除了指定元素之外的所有面板元素
   * @param {Array<HTMLElement>} excludeElements - 要排除的DOM元素数组
   * @returns {Array<HTMLElement>} - 除了指定元素外的所有DOM元素
   */
  getOtherPanelElements(excludeElements) {
    return this.ALL_PANEL_ELEMENTS.filter(
      (element) =>
        !excludeElements.some((excludeElement) => element === excludeElement)
    );
  },
};

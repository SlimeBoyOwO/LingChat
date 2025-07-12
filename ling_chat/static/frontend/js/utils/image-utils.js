export const ImageUtils = {
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
};

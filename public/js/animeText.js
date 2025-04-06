/**
 * 逐字显示文本
 * @param {HTMLElement} element - 要显示文本的元素
 * @param {string} text - 完整文本
 * @param {number} speed - 打字速度（毫秒/字）
 */
function typeWriter(element, text, textSpeed) {
  speed = 50;
  switch (textSpeed) {
    case "medium":
      speed = 50;
      break;
    case "slow":
      speed = 80;
      break;
    case "fast":
      speed = 20;
      break;
  }

  let i = 0;
  element.value = ""; // 清空内容

  const timer = setInterval(() => {
    if (i < text.length) {
      // 逐个字符追加（保留换行和空格）
      element.value += text.charAt(i);
      i++;

      // 自动滚动到底部
      element.scrollTop = element.scrollHeight;
    } else {
      clearInterval(timer); // 动画结束
      element.style.borderRight = "none"; // 移除光标
    }
  }, speed);
}

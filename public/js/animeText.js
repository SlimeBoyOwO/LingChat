class TypeWriter {
  constructor(element) {
    this.element = element;
    this.timer = null;
    this.abortController = null;
    this.speed;
  }

  start(text, numSpeed) {
    // 停止上一个动画
    this.stop();
    if(numSpeed !== null && numSpeed !== undefined && Number.isInteger(numSpeed)) {
      // 如果 speed 是数字，直接使用
      this.speed = numSpeed;
    } else if(numSpeed !== null && numSpeed !== undefined ){
      // 如果 speed 是字符串，尝试转换为数字
      this.speed = Number.parseInt(numSpeed, 10);
    } else {
      // numSpeed 为空或未定义，在控制台警告并使用默认值
      console.warn("速度值未定义，将采用默认速度。");
      this.speed = 50; // 默认值
    }

    this.abortController = new AbortController();
    let i = 0;
    this.element.value = "";

    this.timer = setInterval(() => {
      if (this.abortController.signal.aborted) {
        clearInterval(this.timer);
        return;
      }

      if (i < text.length) {
        this.element.value += text.charAt(i);
        i++;
        this.element.scrollTop = this.element.scrollHeight;
      } else {
        this.stop();
        this.element.style.borderRight = "none";
      }
    }, this.speed);
  }

  stop() {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
    if (this.abortController) {
      this.abortController.abort();
      this.abortController = null;
    }
  }
}

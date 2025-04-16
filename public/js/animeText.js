class TypeWriter {
  constructor(element) {
    this.element = element;
    this.timer = null;
    this.abortController = null;
  }

  start(text, textSpeed) {
    // 停止上一个动画
    this.stop();

    let speed = 50;
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
    }, speed);
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

export class TypeWriter {
  constructor(element) {
    this.element = element;
    this.timer = null;
    this.abortController = null;
    this.speed = 50; // 默认速度
    this.isFinished = false;
    this.onFinishCallback = null;
  }

  start(text, numSpeed) {
    // 停止上一个动画
    this.stop();
    this.isFinished = false;

    if (numSpeed !== null && numSpeed !== undefined) {
      this.speed = Number.isInteger(numSpeed)
        ? numSpeed
        : Number.parseInt(numSpeed, 10) || 50;
    } else {
      console.warn("速度值未定义，将采用默认速度。");
      this.speed = 50;
    }

    this.abortController = new AbortController();
    let i = 0;
    this.element.value = "";
    this.element.textContent = "";

    this.timer = setInterval(() => {
      if (this.abortController.signal.aborted) {
        clearInterval(this.timer);
        return;
      }

      if (i < text.length) {
        this.element.value += text.charAt(i);
        this.element.textContent += text.charAt(i);
        i++;
        this.element.scrollTop = this.element.scrollHeight;
      } else {
        this.finish();
      }
    }, this.speed);
  }

  finish() {
    this.stop();
    this.isFinished = true;
    this.element.style.borderRight = "none";
    if (this.onFinishCallback) {
      this.onFinishCallback();
    }
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
    this.isFinished = false;
  }

  checkFinished() {
    return this.isFinished;
  }

  onFinish(callback) {
    this.onFinishCallback = callback;
  }
}

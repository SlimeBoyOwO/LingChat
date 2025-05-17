import { EMOTION_CONFIG } from "./config.js";
import { DOM } from "../../ui/dom.js";
import EventBus from "../../core/event-bus.js";

export class EmotionController {
  constructor() {
    this.currentEmotion = "normal";
    this.animationEndHandler = this._handleAnimationEnd.bind(this);
    DOM.avatar.container.addEventListener(
      "animationend",
      this.animationEndHandler
    );
  }

  /**
   * 切换表情
   * @param {string} emotion - 表情标识符
   * @param {object} [options] - 额外选项
   * @param {boolean} [options.force=false] - 是否强制重新播放
   */
  setEmotion(emotion, { force = false } = {}) {
    if (!EMOTION_CONFIG[emotion]) {
      console.warn(`未知表情: ${emotion}`);
      return;
    }

    if (this.currentEmotion === emotion && !force) return;

    const config = EMOTION_CONFIG[emotion];
    this._clearCurrentEffects();

    // 更新头像图片
    if (config.avatar && config.avatar !== "none") {
      DOM.avatar.img.src = config.avatar;
    }

    // 处理动画效果
    if (config.animation !== "none") {
      DOM.avatar.container.classList.add(config.animation);
    }

    // 触发气泡效果
    if (config.bubble !== "none") {
      this._showBubbleEffect(config);
    }

    // 播放音效
    if (config.audio !== "none") {
      EventBus.emit("audio:play-effect", {
        type: "emotion",
        src: config.audio,
      });
      // 设置音频源并播放
      console.log(`${emotion} 音频播放ing`);
      console.log(`${config.audio} 路径追踪`);
      DOM.bubbleAudio.src = config.audio;
      DOM.bubbleAudio.load(); // 重新加载音频
      DOM.bubbleAudio.play();
    }

    this.currentEmotion = emotion;
    EventBus.emit("emotion:changed", emotion);
  }

  _clearCurrentEffects() {
    // 移除所有动画类
    Object.values(EMOTION_CONFIG).forEach(({ animation }) => {
      if (animation !== "none") {
        DOM.avatar.container.classList.remove(animation);
      }
    });
  }

  _showBubbleEffect(config) {
    const version = Date.now();

    if (config.bubbleImage === "none") return;

    // 重置动画
    DOM.avatar.bubble.classList.remove("show");
    void DOM.avatar.bubble.offsetWidth;

    // 设置带时间戳的新源
    DOM.avatar.bubble.style.backgroundImage = `url(${config.bubbleImage}??t=${version}#t=0.1)`;

    // 添加对应的表情类
    DOM.avatar.bubble.classList.add(config.bubbleClass);

    // 触发显示
    DOM.avatar.bubble.classList.add("show");

    // 动画结束后隐藏
    setTimeout(() => {
      DOM.avatar.bubble.classList.remove("show");
      DOM.avatar.bubble.classList.remove(config.bubbleClass);
    }, 2000); // 2秒后执行
  }

  _handleAnimationEnd() {
    this._clearCurrentEffects();
    DOM.avatar.container.classList.add("idle-animation");
  }

  destroy() {
    DOM.avatar.container.removeEventListener(
      "animationend",
      this.animationEndHandler
    );
  }
}

import { EMOTION_CONFIG } from "./config";
import { DOM } from "../../ui/dom";
import EventBus from "../../core/event-bus";

export class EmotionController {
  private currentEmotion: string;
  private animationEndHandler: EventListener;

  constructor() {
    this.currentEmotion = "normal";
    this.animationEndHandler = this._handleAnimationEnd.bind(this);
    DOM.avatar.container.addEventListener(
      "animationend",
      this.animationEndHandler
    );
  }

  public setEmotion(emotion: string, { force = false } = {}) {
    if (!EMOTION_CONFIG[emotion]) {
      console.log(`未知表情: ${emotion}`);
      return;
    }

    if (this.currentEmotion === emotion && !force) return;

    const config = EMOTION_CONFIG[emotion];
    this._clearCurrentEffects();

    // 更新头像图片
    if (config.avatar && config.avatar !== "none") {
      try {
        // 显示加载状态
        DOM.avatar.img.classList.add("loading");

        // 创建新的Image对象预加载
        const img = new Image();
        img.onload = () => {
          // 加载完成后替换src
          DOM.avatar.img.src = img.src;
          DOM.avatar.img.classList.remove("loading");
        };
        img.onerror = () => {
          console.log(`加载头像失败: ${config.avatar}`);
          DOM.avatar.img.classList.remove("loading");
          // 可以设置一个默认头像
          DOM.avatar.img.src = "/api/v1/chat/character/get_avatar/正常.png";
        };

        // 添加时间戳防止缓存
        const timestamp = Date.now();
        img.src = `${config.avatar}?t=${timestamp}`;
      } catch (error) {
        console.error("设置表情头像时出错:", error);
        DOM.avatar.img.classList.remove("loading");
      }
    }

    // 处理动画效果
    if (config.animation !== "none") {
      DOM.avatar.container.classList.remove(this.currentEmotion);
      DOM.avatar.container.classList.add(config.animation);
    }

    // 触发气泡效果
    if (config.bubbleImage !== "none") {
      this._showBubbleEffect(config);
    }

    // 播放音效
    if (config.audio !== "none") {
      EventBus.emit("audio:play-effect", {
        type: "emotion",
        src: config.audio,
      });
      // 设置音频源并播放
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

  _showBubbleEffect(config: any) {
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
    DOM.avatar.container.classList.add("normal");
  }

  destroy() {
    DOM.avatar.container.removeEventListener(
      "animationend",
      this.animationEndHandler
    );
  }
}

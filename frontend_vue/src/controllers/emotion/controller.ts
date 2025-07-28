import { EMOTION_CONFIG } from "./config";
import { useGameStore } from "../../stores/modules/game";

export interface EmotionDOM {
  avatar: {
    container: HTMLElement;
    img: HTMLImageElement;
    bubble: HTMLElement;
  };
  bubbleAudio: HTMLAudioElement;
}

export class EmotionController {
  private currentEmotion: string;
  private animationEndHandler: EventListener;
  private DOM: EmotionDOM;

  constructor(DOM: EmotionDOM) {
    this.DOM = DOM;
    const gameStore = useGameStore();
    this.currentEmotion = gameStore.avatar.emotion || "正常";
    this.animationEndHandler = this._handleAnimationEnd.bind(this);
    this.DOM.avatar.container.addEventListener(
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
        this.DOM.avatar.img.classList.add("loading");

        // 创建新的Image对象预加载
        const img = new Image();
        img.onload = () => {
          // 加载完成后替换src
          this.DOM.avatar.img.src = img.src;
          this.DOM.avatar.img.classList.remove("loading");
        };
        img.onerror = () => {
          console.log(`加载头像失败: ${config.avatar}`);
          this.DOM.avatar.img.classList.remove("loading");
          // 可以设置一个默认头像
          this.DOM.avatar.img.src =
            "/api/v1/chat/character/get_avatar/正常.png";
        };

        // 添加时间戳防止缓存
        const timestamp = Date.now();
        img.src = `${config.avatar}?t=${timestamp}`;
      } catch (error) {
        console.error("设置表情头像时出错:", error);
        this.DOM.avatar.img.classList.remove("loading");
      }
    }

    // 处理动画效果
    if (config.animation !== "none") {
      this.DOM.avatar.container.classList.remove(this.currentEmotion);
      this.DOM.avatar.container.classList.add(config.animation);
    }

    // 触发气泡效果
    if (config.bubbleImage !== "none") {
      this._showBubbleEffect(config);
    }

    // 播放音效
    if (config.audio !== "none") {
      // 这里建议用事件或直接操作 audio
      this.DOM.bubbleAudio.src = config.audio;
      this.DOM.bubbleAudio.load();
      this.DOM.bubbleAudio.play();
    }

    this.currentEmotion = emotion;
    // 可以在这里触发 pinia 或 emit 事件
  }

  _clearCurrentEffects() {
    // 移除所有动画类
    Object.values(EMOTION_CONFIG).forEach(({ animation }) => {
      if (animation !== "none") {
        this.DOM.avatar.container.classList.remove(animation);
      }
    });
  }

  _showBubbleEffect(config: any) {
    const version = Date.now();

    if (config.bubbleImage === "none") return;

    // 重置动画
    this.DOM.avatar.bubble.classList.remove("show");
    void this.DOM.avatar.bubble.offsetWidth;

    // 设置带时间戳的新源
    this.DOM.avatar.bubble.style.backgroundImage = `url(${config.bubbleImage}?t=${version}#t=0.1)`;

    // 添加对应的表情类
    this.DOM.avatar.bubble.classList.add(config.bubbleClass);

    // 触发显示
    this.DOM.avatar.bubble.classList.add("show");

    // 动画结束后隐藏
    setTimeout(() => {
      this.DOM.avatar.bubble.classList.remove("show");
      this.DOM.avatar.bubble.classList.remove(config.bubbleClass);
    }, 2000); // 2秒后执行
  }

  _handleAnimationEnd() {
    this._clearCurrentEffects();
    this.DOM.avatar.container.classList.add("normal");
  }

  destroy() {
    this.DOM.avatar.container.removeEventListener(
      "animationend",
      this.animationEndHandler
    );
  }
}

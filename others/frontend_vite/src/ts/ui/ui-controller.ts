// ui-controller.ts
import { DOM } from "./dom.js";
import { TypeWriter } from "./type-writer.js";
import { EmotionController } from "../features/emotion/controller.js";
import { API_CONFIG } from "../core/config.js";
import EventBus from "../core/event-bus.js";
import request from "../core/request.js";

// 定义接口
interface AIInfo {
  ai_name: string;
  ai_subtitle: string;
  user_name: string;
  user_subtitle: string;
  character_id: number;
  thinking_message: string;
  scale: string;
  offset: string;
  bubble_top: string;
  bubble_left: string;
}

interface ChatMessageData {
  content: string;
  motionText?: string;
  emotion?: string;
  originalTag?: string;
  audioFile?: string;
}

export class UIController {
  private emotionSystem: EmotionController;
  private writer: TypeWriter;
  private speed: number;

  public ai_name: string;
  public ai_subtitle: string;
  public user_name: string;
  public user_subtitle: string;
  public think_message: string;

  private userId: number;
  private character_id: number;
  private enable_sound_effects: boolean;

  private handleSend: () => void;
  private handleKeyPress: (e: KeyboardEvent) => void;

  constructor() {
    this.emotionSystem = new EmotionController();
    this.writer = new TypeWriter(DOM.input);
    this.speed = 50;

    this.ai_name = "钦灵";
    this.ai_subtitle = "Slime Studio";
    this.user_name = "可爱的你";
    this.user_subtitle = "Bilibili";
    this.think_message = "灵灵正在思考ing...";

    this.userId = 1;
    this.character_id = 1;
    this.enable_sound_effects = true;

    // 绑定实例方法
    this.handleSend = this.handleSendEvent.bind(this);
    this.handleKeyPress = this.handleKeyPressEvent.bind(this);

    this.init();
  }

  init(): void {
    this.bindEventListeners();
    this.bindEvents();
    this.getAndApplyAIInfo();
  }

  destroy(): void {
    DOM.sendBtn?.removeEventListener("click", this.handleSend);
    document?.removeEventListener("keypress", this.handleKeyPress);
  }

  async getAndApplyAIInfo(): Promise<void> {
    try {
      const data: AIInfo = await request.informationGet(this.userId.toString());

      this.ai_name = data.ai_name;
      this.ai_subtitle = data.ai_subtitle;
      this.user_name = data.user_name;
      this.user_subtitle = data.user_subtitle;
      this.character_id = data.character_id;
      this.think_message = data.thinking_message;

      // 动态设置 transform 和 transform-origin
      DOM.avatar.img.style.transform = `scale(${data.scale})`;
      DOM.avatar.img.style.transformOrigin = `center ${data.offset}%`;

      // 顺便设置预览图像里面的同样的设定
      DOM.image.kousanPreviewImg.style.transform = `scale(${data.scale})`;
      DOM.image.kousanPreviewImg.style.transformOrigin = `center ${data.offset}%`;

      // 设置bubble的css样式中的top和left
      DOM.avatar.bubble.style.top = `${data.bubble_top}%`;
      DOM.avatar.bubble.style.left = `${data.bubble_left}%`;

      this.resetAvatar();
      this.updateSelectedStatus();

      // 发送事件，方便其他地方监听
      EventBus.emit("ui:name-updated", {
        ai_name: this.ai_name,
        ai_subtitle: this.ai_subtitle,
        user_name: this.user_name,
        user_subtitle: this.user_subtitle,
      });
    } catch (error) {
      console.error("读取失败", error);
    }
  }

  resetAvatar(): void {
    DOM.avatar.title.textContent = this.user_name;
    DOM.avatar.subtitle.textContent = this.user_subtitle;

    this.emotionSystem.setEmotion("正常", { force: true });
    DOM.image.kousanPreviewImg.src = `/api/v1/chat/character/get_avatar/正常.png?t=${Date.now()}`;
  }

  private updateSelectedStatus(): void {
    document.querySelectorAll(".character-select-btn").forEach((btn) => {
      if (btn instanceof HTMLElement) {
        btn.classList.remove("selected");
        btn.textContent = "选择"; // 重置按钮文字

        if (btn.dataset.characterId === String(this.character_id)) {
          btn.classList.add("selected");
          btn.textContent = "✓ 已选择"; // 更新选中按钮文字
        }
      }
    });
  }

  bindEventListeners(): void {
    DOM.sendBtn?.addEventListener("click", this.handleSend);
    document?.addEventListener("keypress", this.handleKeyPress);
  }

  bindEvents(): void {
    // 更新角色和信息事件
    EventBus.on("system:character_updated", () => {
      this.getAndApplyAIInfo();
    });

    // 接受消息事件
    EventBus.on("chat:message", (data: ChatMessageData) => {
      DOM.input.placeholder = "";

      let displayText =
        data.motionText && data.motionText !== ""
          ? `${data.content}（${data.motionText}）`
          : data.content;

      // 更新情绪
      if (data.emotion) {
        this.emotionSystem.setEmotion(data.emotion);
        if (data.originalTag) {
          DOM.avatar.emotion.textContent = data.originalTag;
        }
      }

      // 处理音频
      if (data.audioFile && data.audioFile !== "none") {
        this.writer.setSoundEnabled(false);
        DOM.audioPlayer.src = `${API_CONFIG.VOICE.BASE}/${data.audioFile}`;
        DOM.audioPlayer.load();
        DOM.audioPlayer.play();
      } else {
        this.writer.setSoundEnabled(true);
      }

      if (!this.enable_sound_effects) {
        this.writer.setSoundEnabled(false);
      }

      // 显示消息内容
      this.writer.start(displayText, this.speed);
    });

    // 启用输入事件
    EventBus.on("chat:input-enabled", () => {
      DOM.input.placeholder = "输入消息...";
      DOM.input.disabled = false;
      DOM.input.value = "";
      DOM.avatar.title.textContent = this.user_name;
      DOM.avatar.subtitle.textContent = this.user_subtitle;
      DOM.avatar.emotion.textContent = "";
      this.writer.stop();
    });

    // 等待AI回复事件
    EventBus.on("chat:thinking", (isThinking: boolean) => {
      if (isThinking) {
        DOM.input.disabled = true;
        DOM.input.value = "";
        this.emotionSystem.setEmotion("AI思考");
        DOM.input.placeholder = this.think_message;
        DOM.avatar.title.textContent = this.ai_name;
        DOM.avatar.subtitle.textContent = this.ai_subtitle;
        DOM.avatar.emotion.textContent = "";
      } else {
        DOM.input.disabled = false;
      }
    });

    // 停止哔哔声音
    EventBus.on("sound:enable_effect", (enabled: boolean) => {
      this.enable_sound_effects = enabled;
    });

    EventBus.on("text:speed_change", (speed: number) => {
      this.speed = speed;
    });

    // 监听 WebSocket 状态更新
    EventBus.on("connection:open", () => {
      if (DOM.status) DOM.status.textContent = "✅ 已连接服务器";
      if (DOM.sendBtn) DOM.sendBtn.disabled = false;
    });

    EventBus.on("connection:dead", () => {
      if (DOM.status) DOM.status.textContent = "❌ 无法连接服务器";
      if (DOM.sendBtn) DOM.sendBtn.disabled = true;
    });

    EventBus.on("connection:error", (err: Error) => {
      console.error("连接错误：", err);
      if (DOM.status) DOM.status.textContent = "⚠️ 连接异常";
      if (DOM.sendBtn) DOM.sendBtn.disabled = true;
    });
  }

  private handleSendEvent(): void {
    EventBus.emit("ui:send-message", DOM.input.value);
  }

  private handleKeyPressEvent(e: KeyboardEvent): void {
    if (e.key === "Enter") this.handleSend();
  }

  public getCurrentCharacterId(): number {
    return this.character_id;
  }
}

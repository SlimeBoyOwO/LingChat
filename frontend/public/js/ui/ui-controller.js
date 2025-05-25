// ui-controller.js
import { DOM } from "./dom.js";
import { TypeWriter } from "./type-writer.js";
import { EmotionController } from "../features/emotion/controller.js";
import EventBus from "../core/event-bus.js";

export class UIController {
  constructor() {
    this.emotionSystem = new EmotionController();
    this.writer = new TypeWriter(DOM.input);
    this.speed = 50;

    // 绑定实例方法
    this.handleSend = this.handleSend.bind(this);
    this.handleKeyPress = this.handleKeyPress.bind(this);

    this.init();
  }

  init() {
    this.bindEventListeners();
    this.setupGlobalHandlers();
  }

  destroy() {
    DOM.sendBtn?.removeEventListener("click", this.handleSend);
    document?.removeEventListener("keypress", this.handleKeyPress);
  }

  bindEventListeners() {
    DOM.sendBtn?.addEventListener("click", this.handleSend);
    document?.addEventListener("keypress", this.handleKeyPress);
  }

  setupGlobalHandlers() {
    // 接受消息事件
    EventBus.on("chat:message", (data) => {
      DOM.input.placeholder = "";

      let displayText = "";

      if (data.motionText && data.motionText !== "") {
        displayText = data.content + "（" + data.motionText + "）";
      } else {
        displayText = data.content;
      }

      // 显示消息内容
      this.writer.start(displayText, this.speed);

      // 更新情绪
      if (data.emotion) {
        this.emotionSystem.setEmotion(data.emotion);
        DOM.avatar.emotion.textContent = data.originalTag;
      }

      // 处理音频
      if (data.audioFile) {
        DOM.audioPlayer.src = `../audio/${data.audioFile}`;
        console.log(`语音的路径是：../audio/${data.audioFile}`);
        DOM.audioPlayer.load();
        DOM.audioPlayer.play();
      }
    });

    // 启用输入事件
    EventBus.on("chat:input-enabled", () => {
      DOM.input.placeholder = "输入消息...";
      DOM.input.disabled = false;
      DOM.input.value = "";
      DOM.avatar.title.textContent = "可爱的你";
      DOM.avatar.subtitle.textContent = "Bilibili";
      DOM.avatar.emotion.textContent = "";
      this.writer.stop();
    });

    // 等待AI回复事件
    EventBus.on("chat:thinking", (isThinking) => {
      if (isThinking) {
        DOM.input.disabled = true;
        DOM.input.value = "";
        this.emotionSystem.setEmotion("AI思考");
        DOM.input.placeholder = "灵灵正在思考...";
        DOM.avatar.title.textContent = "钦灵";
        DOM.avatar.subtitle.textContent = "Slime Studio";
        DOM.avatar.emotion.textContent = "";
      } else {
        DOM.input.disabled = false;
      }
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

    EventBus.on("connection:error", (err) => {
      console.error("连接错误：", err);
      if (DOM.status) DOM.status.textContent = "⚠️ 连接异常";
      if (DOM.sendBtn) DOM.sendBtn.disabled = true;
    });
  }

  handleSend = () => {
    EventBus.emit("ui:send-message", DOM.input.value);
  };

  handleKeyPress = (e) => {
    if (e.key === "Enter") this.handleSend();
  };
}

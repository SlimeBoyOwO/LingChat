import { DOM } from "../../../ui/dom.js";
import { DomUtils } from "../../../utils/dom-utils";
import { TypeWriter } from "../../../ui/type-writer";
import EventBus from "../../../core/event-bus.js";

export class TextController {
  private maxSpeed: number;
  private message: string;
  private domUtils: typeof DomUtils;
  private typeWriter: TypeWriter;
  private restartTimer: ReturnType<typeof setTimeout> | undefined = undefined;

  constructor() {
    this.maxSpeed = 150;
    this.message = "Ling Chat，测试文本显示速度";
    this.domUtils = DomUtils;
    this.typeWriter = new TypeWriter(DOM.text.testMessage);
    this.typeWriter.setSoundEnabled(false);
    this.restartTimer = undefined;
    this.init();
  }

  private init(): void {
    this.bindEvents();
    this.initSettings();
  }

  private bindEvents(): void {
    DOM.menuText?.addEventListener("click", () => this.showTextPanel());

    DOM.text.speedInput?.addEventListener("change", (e) => {
      if (!(e.target instanceof HTMLInputElement)) return;

      const speed = e.target.value;
      localStorage.setItem("numSpeed", speed);
      this.applyTextSpeed(speed);
    });

    DOM.text.soundEffectToggle.addEventListener("change", function () {
      if (this.checked) {
        EventBus.emit("sound:enable_effect", true);
        console.log("123");
      } else {
        EventBus.emit("sound:enable_effect", false);
        console.log("456");
      }
    });

    EventBus.on("menu:text_panel", () => {
      this.showTextPanel();
    });
  }

  private showTextPanel(): void {
    this.domUtils.showElements([DOM.menuText, DOM.textPage]);
    this.domUtils.hideElements(
      this.domUtils.getOtherPanelElements([DOM.menuText, DOM.textPage])
    );
  }

  private initSettings() {
    // 文字速度
    const savedSpeed = localStorage.getItem("numSpeed") || "50";
    if (DOM.text.speedInput) DOM.text.speedInput.value = savedSpeed;
    this.applyTextSpeed(savedSpeed);

    // 背景
    const savedBg = localStorage.getItem("background") || "night";
    const bgOption = document.querySelector(`.bg-option[data-bg="${savedBg}"]`);
    if (bgOption) bgOption.classList.add("active");
    document.body.classList.add(`bg-${savedBg}`);
  }

  private applyTextSpeed(speed: string) {
    this.maxSpeed = 200 - parseInt(speed);

    EventBus.emit("text:speed_change", this.maxSpeed);
    this.testTextSpeed();
  }

  private testTextSpeed() {
    clearTimeout(this.restartTimer);

    if (!DOM.text.testMessage) return;

    DOM.text.testMessage.textContent = "";
    this.typeWriter.start(this.message, this.maxSpeed);
    this.typeWriter.onFinish(() => {
      this.restartTimer = setTimeout(() => this.testTextSpeed(), 500);
    });
  }
}

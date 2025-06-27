import { DOM } from "../../ui/dom.js";
import { TypeWriter } from "../../ui/type-writer.js";
import { DomUtils } from "../../utils/dom-utils.js";
import EventBus from "../../core/event-bus.js";

export class MenuController {
  constructor(uiController) {
    this.numSpeed = 150;
    this.settingsSpeed = false;
    this.message = "Ling Chat，测试文本显示速度";
    this.restartTimer = null;
    this.domUtils = DomUtils;
    this.uiController = uiController;
    this.typeWriter = new TypeWriter(DOM.text.testMessage);
    this.typeWriter.setSoundEnabled(false);
    this.init();
  }

  init() {
    this.bindMenuEvents();
    this.bindSpeedControl();
    this.bindBackgroundSelection();
    this.initSettings();
  }

  bindMenuEvents() {
    // 主菜单切换
    DOM.menuToggle?.addEventListener("click", () => this.playMenuAnimation());

    // 文本菜单
    DOM.menuText?.addEventListener("click", () => this.showTextPanel());

    // 关闭菜单
    DOM.closeMenu?.addEventListener("click", () => this.closeAllPanels());

    // 启动是否开启对话音效
    DOM.text.soundEffectToggle.addEventListener("change", function () {
      if (this.checked) {
        EventBus.emit("sound:enable_effect", true);
        console.log("123");
      } else {
        EventBus.emit("sound:enable_effect", false);
        console.log("456");
      }
    });
  }

  playMenuAnimation() {
    const bgEffectFront = document.getElementById("frontpage-effect");
    if (!bgEffectFront) {
      this.toggleMainMenu(true);
      return;
    }

    const animElement = document.createElement("img");
    animElement.src = "../pictures/animation/转场.webp?" + Date.now();
    animElement.style.objectFit = "cover";

    bgEffectFront.innerHTML = "";
    bgEffectFront.appendChild(animElement);

    bgEffectFront.style.display = "block";

    DOM.effectAudio.src = "../audio_effects/转场.wav";
    DOM.effectAudio.load(); // 重新加载音频
    DOM.effectAudio.play();

    animElement.onload = () => {
      setTimeout(() => {
        this.toggleMainMenu(true);
      }, 1300);
      setTimeout(() => {
        bgEffectFront.style.display = "none";
      }, 1500);
    };

    setTimeout(() => {
      if (bgEffectFront.style.display !== "none") {
        bgEffectFront.style.display = "none";
        this.toggleMainMenu(true);
      }
    }, 2000);
  }

  toggleMainMenu(show) {
    if (show) {
      DOM.menuContent?.classList.add("show");
      this.showTextPanel();
    } else {
      this.closeAllPanels();
    }
  }

  showTextPanel() {
    this.domUtils.showElements([DOM.menuText, DOM.textPage]);
    this.domUtils.hideElements(
      this.domUtils.getOtherPanelElements([DOM.menuText, DOM.textPage])
    );
  }

  closeAllPanels() {
    this.domUtils.hideMenuElements();

    DOM.menuContent.classList.add("hide");
    setTimeout(() => {
      DOM.menuContent.classList.remove("hide");
      DOM.menuContent.classList.remove("show");
    }, 200);
  }

  bindSpeedControl() {
    DOM.text.speedInput?.addEventListener("change", (e) => {
      const speed = e.target.value;
      localStorage.setItem("numSpeed", speed);
      this.applyTextSpeed(speed);
      this.settingsSpeed = true;
    });
  }

  applyTextSpeed(speed) {
    this.numSpeed = 200 - parseInt(speed);

    this.uiController.speed = this.numSpeed;
    this.testTextSpeed();
  }

  testTextSpeed() {
    this.settingsSpeed = false;
    clearTimeout(this.restartTimer);

    if (!DOM.text.testMessage) return;

    DOM.text.testMessage.textContent = "";
    this.typeWriter.start(this.message, this.numSpeed);
    this.typeWriter.onFinish(() => {
      this.restartTimer = setTimeout(() => this.testTextSpeed(), 500);
    });
  }

  bindBackgroundSelection() {
    document.querySelectorAll(".bg-option").forEach((option) => {
      option.addEventListener("click", () => this.selectBackground(option));
    });
  }

  selectBackground(option) {
    document.querySelectorAll(".bg-option").forEach((bg) => {
      bg.classList.remove("active");
    });

    option.classList.add("active");
    const bgClass = option.dataset.bg;

    document.body.className = "";
    document.body.classList.add(`bg-${bgClass}`);
    localStorage.setItem("background", bgClass);
  }

  initSettings() {
    // 文字速度
    const savedSpeed = localStorage.getItem("numSpeed") || "50";
    if (DOM.speedInput) DOM.speedInput.value = savedSpeed;
    this.applyTextSpeed(savedSpeed);

    // 背景
    const savedBg = localStorage.getItem("background") || "night";
    const bgOption = document.querySelector(`.bg-option[data-bg="${savedBg}"]`);
    if (bgOption) bgOption.classList.add("active");
    document.body.classList.add(`bg-${savedBg}`);
  }
}

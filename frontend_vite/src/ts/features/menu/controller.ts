import { DOM } from "../../ui/dom.js";
import { DomUtils } from "../../utils/dom-utils.js";
import EventBus from "../../core/event-bus.js";

export class MenuController {
  private domUtils: typeof DomUtils;

  constructor() {
    this.domUtils = DomUtils;
    this.init();
  }

  private init() {
    this.bindMenuEvents();
  }

  private bindMenuEvents() {
    // 主菜单切换
    DOM.menuToggle?.addEventListener("click", () => this.playMenuAnimation());

    // 关闭菜单
    DOM.closeMenu?.addEventListener("click", () => this.closeAllPanels());
  }

  private playMenuAnimation() {
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

  private toggleMainMenu(show: boolean) {
    if (show) {
      DOM.menuContent?.classList.add("show");
      EventBus.emit("menu:text_panel", {});
    } else {
      this.closeAllPanels();
    }
  }

  private closeAllPanels() {
    this.domUtils.hideMenuElements();

    DOM.menuContent.classList.add("hide");
    setTimeout(() => {
      DOM.menuContent.classList.remove("hide");
      DOM.menuContent.classList.remove("show");
    }, 200);
  }
}

import { DOM } from "../../ui/dom.js";
import { DomUtils } from "../../utils/dom-utils.js";

export class SoundController {
  constructor() {
    this.processing = false;
    this.domUtils = DomUtils;
    this.init();
  }

  init() {
    this.bindEvents();
  }

  bindEvents() {
    if (!DOM.menuSound) return;

    DOM.menuSound.addEventListener("click", () => this.toggleSoundPanel());
  }

  toggleSoundPanel() {
    if (this.processing) return;
    this.processing = true;

    requestAnimationFrame(() => {
      // 显示声音相关元素
      this.domUtils.showElements([
        DOM.menuContent,
        DOM.menuSound,
        DOM.soundPage,
      ]);

      // 隐藏其他面板元素
      this.domUtils.hideElements([
        DOM.menuImage,
        DOM.imagePage,
        DOM.menuSave,
        DOM.history.toggle,
        DOM.history.content,
        DOM.history.clearBtn,
        DOM.menuText,
        DOM.textPage,
        DOM.savePage,
      ]);

      setTimeout(() => {
        this.processing = false;
      }, 300);
    });
  }

  // 可扩展的音频控制方法
  playBubbleSound() {
    if (DOM.bubbleAudio) {
      DOM.bubbleAudio.currentTime = 0;
      DOM.bubbleAudio.play();
    }
  }

  stopAllSounds() {
    [DOM.audioPlayer, DOM.bubbleAudio].forEach((player) => {
      if (player) {
        player.pause();
        player.currentTime = 0;
      }
    });
  }
}

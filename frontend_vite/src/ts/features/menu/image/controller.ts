import { DOM } from "../../../ui/dom.js";
import { DomUtils } from "../../../utils/dom-utils.js";
import { KousanManager } from "./kuosan-manager.js";
import request from "../../../core/request.js";

interface Background {
  title: string;
  url: string;
  time: string;
}

export class ImageController {
  private backgroundList: HTMLElement;
  private domUtils: typeof DomUtils;
  private currentSelectedCard: Element | null;
  private kousanManager: KousanManager;

  constructor() {
    this.backgroundList = document.getElementById(
      "background-list"
    ) as HTMLElement;
    this.domUtils = DomUtils;
    this.currentSelectedCard = null;
    this.kousanManager = new KousanManager();
    this.init();
  }

  async init(): Promise<void> {
    try {
      await this.refreshBackground();
      this.bindEvents();

      const savedBg = localStorage.getItem("selectedBackground");
      if (savedBg) {
        const card = this.findCardByUrl(savedBg);
        if (card) {
          this.selectBackground(card, savedBg);
        }
      } else {
        const randomCard = this.getRandomCard();
        if (randomCard) {
          const btn = randomCard.querySelector(
            ".background-select-btn"
          ) as HTMLElement;
          if (btn && btn.dataset.backgroundUrl) {
            this.selectBackground(randomCard, btn.dataset.backgroundUrl);
            console.log("已选随机背景");
          }
        }
      }
    } catch (error) {
      console.error("加载背景图片失败", error);
    }
  }

  async refreshBackground(): Promise<void> {
    const backgrounds = await this.fetchBackgrounds();
    this.renderBackgrounds(backgrounds);
  }

  getRandomCard(): Element | null {
    const cards = this.backgroundList.querySelectorAll(".background-card");
    if (cards.length === 0) {
      return null;
    }
    const randomIndex = Math.floor(Math.random() * cards.length);
    return cards[randomIndex];
  }

  findCardByUrl(url: string): Element | null {
    const cards = this.backgroundList.querySelectorAll(".background-card");
    for (const card of cards) {
      const btn = card.querySelector(".background-select-btn");
      if (
        btn &&
        btn instanceof HTMLElement &&
        btn.dataset.backgroundUrl === url
      ) {
        return card;
      }
    }
    return null;
  }

  bindEvents(): void {
    DOM.menuImage?.addEventListener("click", () => this.showImagePanel());

    this.backgroundList.addEventListener("click", (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      const selectBtn = target.closest(".background-select-btn");
      if (selectBtn && selectBtn instanceof HTMLElement) {
        const card = selectBtn.closest(".background-card");
        const bgUrl = selectBtn.dataset.backgroundUrl;
        if (card && bgUrl) {
          this.selectBackground(card, bgUrl);
        }
      }
    });

    DOM.image.uploadBgBtn?.addEventListener("click", () =>
      DOM.image.uploadBgInput?.click()
    );

    DOM.image.uploadBgInput?.addEventListener("change", (e: Event) => {
      const target = e.target as HTMLInputElement;
      const file = target.files?.[0];
      if (!file) return;

      this.uploadBackground(file);
    });
  }

  showImagePanel(): void {
    this.domUtils.showElements([DOM.menuImage, DOM.imagePage]);
    this.domUtils.hideElements(
      this.domUtils.getOtherPanelElements([DOM.menuImage, DOM.imagePage])
    );
  }

  async fetchBackgrounds(): Promise<Background[]> {
    try {
      const { data } = await request.backgroundList();
      return data.map((background: any) => ({
        title: background.title,
        url: background.image_path
          ? `/api/v1/chat/background/background_file/${encodeURIComponent(
              background.image_path
            )}`
          : "../pictures/background/default.png",
        time: background.modified_time,
      }));
    } catch (error) {
      console.error("获取背景列表失败:", error);
      return [];
    }
  }

  renderBackgrounds(backgrounds: Background[]): void {
    this.backgroundList.innerHTML = backgrounds
      .map((background) => this.createBackgroundCard(background))
      .join("");
  }

  createBackgroundCard(background: Background): string {
    return `
      <div class="background-card">
        <div class="background-image-container">
          <img src="${background.url}" alt="${background.title}" class="background-image">
        </div>
        <div class="background-title" data-title="${background.title}">
          <button class="background-select-btn" data-background-url="${background.url}">选择</button>
        </div>
      </div>
    `;
  }

  selectBackground(card: Element, bgUrl: string): void {
    if (this.currentSelectedCard) {
      this.currentSelectedCard.classList.remove("selected");
      const prevBtn = this.currentSelectedCard.querySelector(
        ".background-select-btn"
      );
      if (prevBtn && prevBtn instanceof HTMLElement) {
        prevBtn.textContent = "选择";
        prevBtn.style.backgroundColor = "#4f46e5";
      }
    }

    card.classList.add("selected");
    const selectBtn = card.querySelector(
      ".background-select-btn"
    ) as HTMLElement;
    if (selectBtn) {
      selectBtn.textContent = "已选中";
      selectBtn.style.backgroundColor = "#10b981";
    }

    DOM.galNormalContainer.style.backgroundImage = `url(${bgUrl})`;
    DOM.galNormalContainer.style.backgroundSize = "cover";
    DOM.galNormalContainer.style.backgroundPosition = "center";
    DOM.galNormalContainer.style.backgroundAttachment = "fixed";
    DOM.galNormalContainer.style.backgroundRepeat = "no-repeat";

    this.kousanManager.updatePreviewBackground(bgUrl);
    this.currentSelectedCard = card;
    localStorage.setItem("selectedBackground", bgUrl);
  }

  async uploadBackground(file: File): Promise<void> {
    const fileName = file.name;
    const fileExt = fileName.slice(fileName.lastIndexOf(".")).toLowerCase();

    const allowedExts = [
      ".jpg",
      ".png",
      ".webp",
      ".bmp",
      ".svg",
      ".tif",
      ".gif",
    ];
    if (!allowedExts.includes(fileExt)) {
      alert("请上传支持的图片格式: " + allowedExts.join(", "));
      return;
    }

    const formData = new FormData();
    formData.append("file", file);
    formData.append("name", fileName);

    const response = await fetch("/api/v1/chat/background/upload", {
      method: "POST",
      body: formData,
    });

    if (!response.ok) throw new Error("上传失败");

    await this.refreshBackground();
  }
}

import { DOM } from "../../ui/dom.js";
import { DomUtils } from "../../utils/dom-utils.js";
import { KousanManager } from "./kuosan-manager.js";
import request from "../../core/request.js";

export class ImageController {
  constructor() {
    this.backgroundList = document.getElementById("background-list");
    this.domUtils = DomUtils;
    this.currentSelectedCard = null; // 跟踪当前选中的卡片
    this.kousanManager = new KousanManager();
    this.init();
  }

  async init() {
    try {
      await this.refreshBackground();
      this.bindEvents();

      // 从本地存储恢复选中的背景
      const savedBg = localStorage.getItem("selectedBackground");
      if (savedBg) {
        const card = this.findCardByUrl(savedBg);
        if (card) {
          this.selectBackground(card, savedBg);
        }
      } else {
        const randomCard = this.getRandomCard();
        if (randomCard) {
          this.selectBackground(
            randomCard,
            randomCard.querySelector(".background-select-btn").dataset
              .backgroundUrl
          );
          console.log("已选随机背景");
        }
      }
    } catch (error) {
      console.error("加载背景图片失败", error);
    }
  }

  async refreshBackground() {
    const backgrounds = await this.fetchBackgrounds();
    this.renderBackgrounds(backgrounds);
  }

  getRandomCard() {
    const cards = this.backgroundList.querySelectorAll(".background-card");
    if (cards.length === 0) {
      return null; // 如果没有卡片，返回null
    }
    const randomIndex = Math.floor(Math.random() * cards.length);
    return cards[randomIndex];
  }

  findCardByUrl(url) {
    const cards = this.backgroundList.querySelectorAll(".background-card");
    for (const card of cards) {
      const btn = card.querySelector(".background-select-btn");
      if (btn && btn.dataset.backgroundUrl === url) {
        return card;
      }
    }
    return null;
  }

  bindEvents() {
    // 图片菜单点击事件
    DOM.menuImage?.addEventListener("click", () => this.showImagePanel());

    // 添加背景选择事件委托
    this.backgroundList.addEventListener("click", (e) => {
      const selectBtn = e.target.closest(".background-select-btn");
      if (selectBtn) {
        const card = selectBtn.closest(".background-card");
        const bgUrl = selectBtn.dataset.backgroundUrl;
        this.selectBackground(card, bgUrl);
      }
    });

    // 背景上传处理
    DOM.image.uploadBgBtn?.addEventListener("click", () =>
      DOM.image.uploadBgInput?.click()
    );

    // 自定义背景上传处理
    DOM.image.uploadBgInput?.addEventListener("change", (e) => {
      const file = e.target.files[0];
      if (!file) return;

      // TODO 设定上传
      this.uploadBackground(file);
    });
  }

  showImagePanel() {
    this.domUtils.showElements([DOM.menuImage, DOM.imagePage]);
    this.domUtils.hideElements(
      this.domUtils.getOtherPanelElements([DOM.menuImage, DOM.imagePage])
    );
  }

  async fetchBackgrounds() {
    try {
      const { data } = await request.backgroundList();
      return data.map((background) => ({
        title: background.title,
        // 使用新的文件获取接口
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

  renderBackgrounds(backgrounds) {
    this.backgroundList.innerHTML = backgrounds
      .map((background) => this.createBackgroundCard(background))
      .join("");
  }

  createBackgroundCard(background) {
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

  selectBackground(card, bgUrl) {
    // 移除之前选中卡片的样式
    if (this.currentSelectedCard) {
      this.currentSelectedCard.classList.remove("selected");
      const prevBtn = this.currentSelectedCard.querySelector(
        ".background-select-btn"
      );
      if (prevBtn) {
        prevBtn.textContent = "选择";
        prevBtn.style.backgroundColor = "#4f46e5";
      }
    }

    // 设置新选中卡片的样式
    card.classList.add("selected");
    const selectBtn = card.querySelector(".background-select-btn");
    if (selectBtn) {
      selectBtn.textContent = "已选中";
      selectBtn.style.backgroundColor = "#10b981"; // 绿色表示已选中
    }

    // 更新背景图片
    document.body.style.backgroundImage = `url(${bgUrl})`;
    document.body.style.backgroundSize = "cover";
    document.body.style.backgroundPosition = "center";
    document.body.style.backgroundAttachment = "fixed";
    document.body.style.backgroundRepeat = "no-repeat";

    // 让预览区也更新
    this.kousanManager.updatePreviewBackground(bgUrl);

    // 保存当前选中的卡片
    this.currentSelectedCard = card;

    // 这里可以添加保存到本地存储的逻辑
    localStorage.setItem("selectedBackground", bgUrl);
  }

  async uploadBackground(file) {
    const fileName = file.name;
    const fileExt = fileName.slice(fileName.lastIndexOf(".")).toLowerCase();

    // 验证文件类型
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
    // 添加 name 参数（使用文件名作为默认值）
    formData.append("name", fileName);

    const response = await fetch("/api/v1/chat/background/upload", {
      method: "POST",
      body: formData,
    });

    if (!response.ok) throw new Error("上传失败");

    this.refreshBackground();
  }
}

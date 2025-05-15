import { DOM } from "../../ui/dom.js";
import { DomUtils } from "../../utils/dom-utils.js";

export class SaveController {
  constructor() {
    this.processing = false;
    this.domUtils = DomUtils;
    this.init();
  }

  init() {
    this.bindEvents();
  }

  bindEvents() {
    if (!DOM.menuSave) return;

    DOM.menuSave.addEventListener("click", () => this.toggleSavePanel());
  }

  async toggleSavePanel() {
    if (this.processing) return;
    this.processing = true;

    requestAnimationFrame(async () => {
      // 显示存档相关元素
      this.domUtils.showElements([DOM.menuContent, DOM.menuSave, DOM.savePage]);

      // 隐藏其他面板元素
      this.domUtils.hideElements([
        DOM.menuImage,
        DOM.imagePage,
        DOM.history.toggle,
        DOM.history.content,
        DOM.history.clearBtn,
        DOM.menuText,
        DOM.textPage,
        DOM.soundPage,
      ]);

      // 👇 加载用户对话并插入到存档列表
      await this.loadUserConversations();

      setTimeout(() => {
        this.processing = false;
      }, 300);
    });
  }

  async loadUserConversations(page = 1, pageSize = 10) {
    const container = document.getElementById("conversation-list");
    if (!container) return;

    try {
      const response = await fetch(
        `/api/v1/chat/history/list?user_id=1&page=${page}&page_size=${pageSize}`
      );
      const result = await response.json();

      if (result.code !== 200) {
        console.error("获取对话失败", result);
        container.innerHTML = "<p>加载失败</p>";
        return;
      }

      container.innerHTML = ""; // 清空旧数据

      result.data.conversations.forEach((convo) => {
        const createdAt = new Date(convo.created_at);
        const dateStr = `${createdAt.getFullYear()}.${
          createdAt.getMonth() + 1
        }.${createdAt.getDate()}`;
        const title = convo.title || "未命名对话";

        const item = document.createElement("div");
        item.className = "save-item";
        item.innerHTML = `
          <div class="save-info">
            <span class="save-date">${dateStr}</span>
            <span class="save-title">${title}</span>
          </div>
          <div class="save-actions">
            <button class="save-btn load-btn" data-id="${convo.id}">读档</button>
            <button class="save-btn save-btn" data-id="${convo.id}">存档</button>
          </div>
        `;
        container.appendChild(item);
      });
    } catch (error) {
      console.error("加载失败", error);
      container.innerHTML = "<p>加载出错</p>";
    }
  }
}

export class ConversationLoader {
  constructor(containerId, userId, options = {}) {
    this.container = document.getElementById(containerId);
    this.userId = userId;
    this.options = {
      pageSize: 10,
      ...options,
    };

    if (!this.container) {
      console.error(`容器元素 #${containerId} 未找到`);
      return;
    }

    this.bindEvents();
  }

  async loadConversations(page = 1) {
    if (!this.container) return;

    try {
      this.showLoading();

      const response = await this.fetchConversations(page);
      const result = await response.json();

      if (result.code !== 200) {
        throw new Error(result.message || "获取对话失败");
      }

      this.renderConversations(result.data.conversations);
    } catch (error) {
      console.error("加载失败", error);
      this.showError("加载失败");
    }
  }

  async fetchConversations(page) {
    const { pageSize } = this.options;
    return fetch(
      `/api/v1/chat/history/list?user_id=${this.userId}&page=${page}&page_size=${pageSize}`
    );
  }

  renderConversations(conversations) {
    this.container.innerHTML = "";

    if (!conversations || conversations.length === 0) {
      this.showEmpty();
      return;
    }

    conversations.forEach((convo) => {
      const item = this.createConversationItem(convo);
      this.container.appendChild(item);
    });
  }

  createConversationItem(convo) {
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

    this.bindItemEvents(item, convo);
    return item;
  }

  bindItemEvents(item, convo) {
    item.querySelector(".load-btn").addEventListener("click", async () => {
      await this.loadConversationDetail(convo.id);
    });

    // 可以在这里添加存档按钮的事件处理
  }

  async loadConversationDetail(convoId) {
    try {
      const response = await fetch(
        `/api/v1/chat/history/load?user_id=${this.userId}&conversation_id=${convoId}`
      );
      const result = await response.json();

      if (result.code !== 200) {
        throw new Error(result.message || "读取失败");
      }

      const messages = result.data.messages || [];
      this.onConversationLoaded(messages);
    } catch (error) {
      console.error("读取失败", error);
      this.onLoadError(error);
    }
  }

  bindEvents() {
    // 可以在这里绑定分页按钮等事件
  }

  showLoading() {
    this.container.innerHTML = "<p>加载中...</p>";
  }

  showError(message) {
    this.container.innerHTML = `<p>${message}</p>`;
  }

  showEmpty() {
    this.container.innerHTML = "<p>暂无对话记录</p>";
  }

  // 以下方法可以被子类覆盖或通过事件监听实现
  onConversationLoaded(messages) {
    console.log("对话内容：", messages);
    // 实际应用中可以触发自定义事件或回调
  }

  onLoadError(error) {
    console.error("加载对话详情失败", error);
    // 可以显示错误提示等
  }
}

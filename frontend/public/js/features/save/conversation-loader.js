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
    this.loadConversations(1); // 初始加载第一页
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
    item.className = "conversation-item";
    item.innerHTML = `
        <div class="conversation-info">
          <span class="conversation-date">${dateStr}</span>
          <span class="conversation-title">${title}</span>
        </div>
        <div class="conversation-actions">
          <button class="action-btn load-btn" data-id="${convo.id}">读取</button>
          <button class="action-btn save-btn" data-id="${convo.id}">保存</button>
          <button class="action-btn delete-btn" data-id="${convo.id}">删除</button>
        </div>
      `;

    this.bindItemEvents(item, convo);
    return item;
  }

  bindItemEvents(item, convo) {
    // 读取按钮事件
    item.querySelector(".load-btn").addEventListener("click", async () => {
      await this.loadUserConversation(convo.id);
    });

    // 保存按钮事件
    item.querySelector(".save-btn").addEventListener("click", async () => {
      await this.saveUserConversation(convo.id);
    });

    // 删除按钮事件
    item.querySelector(".delete-btn").addEventListener("click", async () => {
      if (confirm("确定要删除这个对话吗？")) {
        await this.deleteUserConversation(convo.id);
      }
    });
  }

  async loadUserConversation(convoId) {
    try {
      const response = await fetch(
        `/api/v1/chat/history/load?user_id=${this.userId}&conversation_id=${convoId}`
      );
      const result = await response.json();

      if (result.code !== 200) {
        throw new Error(result.message || "读取失败");
      }

      this.onConversationLoaded(result.data);
    } catch (error) {
      console.error("读取失败", error);
      this.onLoadError(error);
    }
  }

  async saveUserConversation(convoId) {
    try {
      const response = await fetch("/api/v1/chat/history/save", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          user_id: this.userId,
          conversation_id: convoId,
        }),
      });

      const result = await response.json();

      if (result.code !== 200) {
        throw new Error(result.message || "保存失败");
      }

      alert("对话保存成功！");
      this.loadConversations(); // 刷新列表
    } catch (error) {
      console.error("保存失败", error);
      alert(`保存失败: ${error.message}`);
    }
  }

  async deleteUserConversation(convoId) {
    try {
      const response = await fetch("/api/v1/chat/history/delete", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          user_id: this.userId,
          conversation_id: convoId,
        }),
      });

      const result = await response.json();

      if (result.code !== 200) {
        throw new Error(result.message || "删除失败");
      }

      alert("对话删除成功！");
      this.loadConversations(); // 刷新列表
    } catch (error) {
      console.error("删除失败", error);
      alert(`删除失败: ${error.message}`);
    }
  }

  async createUserConversation() {
    const titleInput = document.getElementById("new-convo-title");
    const title = titleInput.value.trim();

    if (!title) {
      alert("请输入对话标题！");
      return;
    }

    try {
      const response = await fetch("/api/v1/chat/history/create", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          user_id: this.userId,
          title: title,
        }),
      });

      const result = await response.json();

      if (result.code !== 200) {
        throw new Error(result.message || "创建失败");
      }

      alert("对话创建成功！");
      titleInput.value = "";
      this.loadConversations(); // 刷新列表
    } catch (error) {
      console.error("创建失败", error);
      alert(`创建失败: ${error.message}`);
    }
  }

  bindEvents() {
    const createBtn = document.getElementById("create-convo-btn");
    if (createBtn) {
      createBtn.addEventListener("click", () => this.createUserConversation());
    }
  }

  showLoading() {
    this.container.innerHTML = "<div class='loading'>加载中...</div>";
  }

  showError(message) {
    this.container.innerHTML = `<div class='error'>${message}</div>`;
  }

  showEmpty() {
    this.container.innerHTML = "<div class='empty'>暂无对话记录</div>";
  }

  // 可被子类覆盖的方法
  onConversationLoaded(data) {
    console.log("对话内容已加载:", data);
    // 实际应用中应该触发一个事件或调用回调
    // 例如: this.options.onConversationLoaded?.(data);
  }

  onLoadError(error) {
    console.error("加载对话详情失败", error);
    alert(`加载失败: ${error.message}`);
  }
}

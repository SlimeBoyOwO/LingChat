import { DOM } from "../../ui/dom.js";
import request from "../../core/request.js";

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

  loadConversations(page = 1) {
    if (!this.container) return;

      this.showLoading();

      return this.fetchConversations(page)
      .then(list => {
        this.renderConversations(list)
      })
      .catch(error => {
        console.error("加载失败", error);
        this.showError("加载失败");
      }) 
  }

  fetchConversations(page) {
    const { pageSize } = this.options;
    return request.historyList(this.userId, page, pageSize);
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
    const createdAt = new Date(convo.updated_at);
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

  loadUserConversation(convoId) {
    return request.historyLoad(this.userId, convoId)
    .then(data => {
      this.onConversationLoaded(data);
    })
    .catch(error => {
      console.log(error);
      this.onLoadError(error);
    })
  }

  saveUserConversation(convoId) {
    return request.historySave(this.userId, convoId)
    .then(id => {
      this.loadConversations();
    })
    .catch(error => {
      console.log(error);
    })
  }

  deleteUserConversation(convoId) {
    return request.historyDelete(this.userId, convoId)
    .then(id => {
      this.loadConversations();
    })
    .catch(error => {
      console.log(error);
    })
  }

  createUserConversation() {
    const titleInput = document.getElementById("new-convo-title");
    const title = titleInput.value.trim();

    if (!title) {
      alert("请输入对话标题！");
      return;
    }

    return request.historyCreate(this.userId, title)
    .then(id => {
      titleInput.value = "";
      this.loadConversations();
    })
    .catch((error) => {
      console.log(error);
    });
  }

  bindEvents() {
    const createBtn = document.getElementById("create-convo-btn");
    if (createBtn) {
      createBtn.addEventListener("click", () => this.createUserConversation());
    }

    // 文件上传处理
    DOM.save.uploadBtn.addEventListener("click", () => {
      DOM.save.uploadBtn.click();
    });

    // 当用户选择文件后触发上传处理
    DOM.save.fileInput.addEventListener("change", async () => {
      await this.uploadUserLogFile();
    });
  }

  async uploadUserLogFile() {
    console.log("触发了点击事件");
    const fileInput = document.getElementById("log-upload");
    const statusDiv = document.getElementById("upload-status");

    if (!fileInput.files.length) {
      statusDiv.textContent = "请先选择文件";
      return;
    }

    const file = fileInput.files[0];

    try {
      statusDiv.textContent = "正在处理文件...";

      // 读取文件内容
      const text = await file.text();

      // 发送到后端处理
      const result = await request.histortyInput(this.userId, text, file.name);
      statusDiv.textContent = `成功导入 ${result.processed_count} 条对话记录`;
      // 刷新对话列表
      this.loadConversations();
    } catch (error) {
      statusDiv.textContent = `上传失败: ${error}`;
      console.error("上传错误:", error);
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

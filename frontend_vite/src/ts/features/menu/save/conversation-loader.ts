import { DOM } from "../../../ui/dom";
import request from "../../../core/request";

interface Conversation {
  id: string;
  title: string;
  updated_at: string;
  // 可以根据实际接口添加更多字段
}

interface ConversationLoaderOptions {
  pageSize?: number;
  onConversationLoaded?: (data: any) => void;
  // 可以添加更多配置选项
}

export class ConversationLoader {
  private container: HTMLElement | null;
  private userId: string;
  private options: ConversationLoaderOptions;

  constructor(
    containerId: string,
    userId: string,
    options: ConversationLoaderOptions = {}
  ) {
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

  async loadConversations(page: number = 1): Promise<void> {
    if (!this.container) return Promise.resolve();

    this.showLoading();

    try {
      const list = await this.fetchConversations(page);
      this.renderConversations(list);
    } catch (error) {
      console.error("加载失败", error);
      this.showError("加载失败");
    }
  }

  fetchConversations(page: number): Promise<Conversation[]> {
    let { pageSize } = this.options;
    if (pageSize === undefined) pageSize = 10;
    return request.historyList(this.userId, page, pageSize);
  }

  renderConversations(conversations: Conversation[]): void {
    if (!this.container) return;

    this.container.innerHTML = "";

    if (!conversations || conversations.length === 0) {
      this.showEmpty();
      return;
    }

    conversations.forEach((convo: Conversation) => {
      const item = this.createConversationItem(convo);
      this.container?.appendChild(item);
    });
  }

  createConversationItem(convo: Conversation): HTMLElement {
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

  bindItemEvents(item: HTMLElement, convo: Conversation): void {
    // 读取按钮事件
    const loadBtn = item.querySelector<HTMLButtonElement>(".load-btn");
    if (loadBtn) {
      loadBtn.addEventListener("click", async () => {
        await this.loadUserConversation(convo.id);
      });
    }

    // 保存按钮事件
    const saveBtn = item.querySelector<HTMLButtonElement>(".save-btn");
    if (saveBtn) {
      saveBtn.addEventListener("click", async () => {
        await this.saveUserConversation(convo.id);
      });
    }

    // 删除按钮事件
    const deleteBtn = item.querySelector<HTMLButtonElement>(".delete-btn");
    if (deleteBtn) {
      deleteBtn.addEventListener("click", async () => {
        if (confirm("确定要删除这个对话吗？")) {
          await this.deleteUserConversation(convo.id);
        }
      });
    }
  }

  async loadUserConversation(convoId: string): Promise<void> {
    try {
      const data = await request.historyLoad(this.userId, convoId);
      this.onConversationLoaded(data);
    } catch (error) {
      console.log(error);
      this.onLoadError(error);
    }
  }

  async saveUserConversation(convoId: string): Promise<void> {
    try {
      await request.historySave(this.userId, convoId);
      this.loadConversations();
    } catch (error) {
      console.log(error);
    }
  }

  async deleteUserConversation(convoId: string): Promise<void> {
    try {
      await request.historyDelete(this.userId, convoId);
      this.loadConversations();
    } catch (error) {
      console.log(error);
    }
  }

  async createUserConversation(): Promise<void> {
    const titleInput = document.getElementById(
      "new-convo-title"
    ) as HTMLInputElement;
    const title = titleInput.value.trim();

    if (!title) {
      alert("请输入对话标题！");
      return Promise.resolve();
    }

    try {
      await request.historyCreate(this.userId, title);
      titleInput.value = "";
      this.loadConversations();
    } catch (error) {
      console.log(error);
    }
  }

  bindEvents(): void {
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

  async uploadUserLogFile(): Promise<void> {
    console.log("触发了点击事件");
    const fileInput = document.getElementById("log-upload") as HTMLInputElement;
    const statusDiv = document.getElementById("upload-status");

    if (!fileInput?.files?.length) {
      if (statusDiv) statusDiv.textContent = "请先选择文件";
      return;
    }

    const file = fileInput.files[0];

    try {
      if (statusDiv) statusDiv.textContent = "正在处理文件...";

      // 读取文件内容
      const text = await file.text();

      // 发送到后端处理
      const result = await request.historyInput(this.userId, text, file.name);
      if (statusDiv)
        statusDiv.textContent = `成功导入 ${result.processed_count} 条对话记录`;
      // 刷新对话列表
      this.loadConversations();
    } catch (error) {
      if (statusDiv)
        statusDiv.textContent = `上传失败: ${
          error instanceof Error ? error.message : String(error)
        }`;
      console.error("上传错误:", error);
    }
  }

  showLoading(): void {
    if (this.container) {
      this.container.innerHTML = "<div class='loading'>加载中...</div>";
    }
  }

  showError(message: string): void {
    if (this.container) {
      this.container.innerHTML = `<div class='error'>${message}</div>`;
    }
  }

  showEmpty(): void {
    if (this.container) {
      this.container.innerHTML = "<div class='empty'>暂无对话记录</div>";
    }
  }

  // 可被子类覆盖的方法
  onConversationLoaded(data: any): void {
    console.log("对话内容已加载:", data);
    this.options.onConversationLoaded?.(data);
  }

  onLoadError(error: Error | unknown): void {
    console.error("加载对话详情失败", error);
    if (error instanceof Error) alert(`加载失败: ${error.message}`);
  }
}

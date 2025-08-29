import { DOM } from "../../ui/dom.js";
import { DomUtils } from "../../utils/dom-utils.js";
import EventBus from "../../core/event-bus.js";

export class HistoryManager {
  constructor() {
    this.conversationHistory =
      JSON.parse(localStorage.getItem("chatHistory")) || [];
    this.currentConversation = {
      userMessage: "",
      aiResponses: [],
    };
    this.domUtils = DomUtils;
    this.ai_name = "默认";
    this.ai_subtitle = "默认";
    this.user_name = "默认";
    this.user_subtitle = "默认";

    this.init();
  }

  init() {
    this.bindEvents();
    this.render();
  }

  bindEvents() {
    DOM.history.clearBtn.addEventListener(
      "click",
      this.handleClearHistory.bind(this)
    );

    DOM.history.toggle.addEventListener(
      "click",
      this.toggleHistoryPanel.bind(this)
    );

    EventBus.on("ui:name-updated", (names) => {
      this.ai_name = names.ai_name;
      this.ai_subtitle = names.ai_subtitle;
      this.user_name = names.user_name;
      this.user_subtitle = names.user_subtitle;
      this.render();
    });
  }

  handleClearHistory() {
    if (confirm("确定要清空所有历史记录吗？")) {
      this.conversationHistory = [];
      this.saveToStorage();
      this.render();
    }
  }

  toggleHistoryPanel() {
    // 切换历史面板显示
    this.domUtils.showElements([
      DOM.history.toggle,
      DOM.history.content,
      DOM.history.clearBtn,
    ]);
    this.domUtils.hideElements(
      this.domUtils.getOtherPanelElements([
        DOM.history.toggle,
        DOM.history.content,
        DOM.history.clearBtn,
      ])
    );

    this.render();
  }

  render() {
    if (!DOM.history.list) return;

    DOM.history.list.innerHTML =
      this.conversationHistory.length === 0
        ? "<p>暂无历史记录</p>"
        : this.conversationHistory
            .map(
              (conv, index) => `
          <div class="history-item">
            <p><strong>${this.user_name}:</strong> ${conv.userMessage}</p>
            ${conv.aiResponses
              .map((res) => `<p><strong>${this.ai_name}:</strong> ${res}</p>`)
              .join("")}
          </div>
        `
            )
            .join("");
  }

  addMessage(userMessage, aiResponse, end = false) {
    if (userMessage && !this.currentConversation.userMessage) {
      this.currentConversation = {
        userMessage,
        aiResponses: [],
      };
    }

    if (aiResponse) {
      this.currentConversation.aiResponses.push(aiResponse);
    }

    if (
      end &&
      this.currentConversation.userMessage &&
      this.currentConversation.aiResponses.length > 0
    ) {
      this.conversationHistory.unshift({
        userMessage: this.currentConversation.userMessage,
        aiResponses: [...this.currentConversation.aiResponses],
      });

      if (this.conversationHistory.length > 50) {
        this.conversationHistory.pop();
      }

      this.saveToStorage();
      this.currentConversation = { userMessage: "", aiResponses: [] };

      if (DOM.history.content?.classList.contains("show")) {
        this.render();
      }
    }
  }

  saveToStorage() {
    localStorage.setItem(
      "chatHistory",
      JSON.stringify(this.conversationHistory)
    );
  }
}

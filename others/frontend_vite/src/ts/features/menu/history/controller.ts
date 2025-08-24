import { DOM } from "../../../ui/dom.js";
import { DomUtils } from "../../../utils/dom-utils.js";
import EventBus from "../../../core/event-bus.js";

interface Conversation {
  userMessage: string;
  aiResponses: string[];
}

interface Names {
  ai_name: string;
  ai_subtitle: string;
  user_name: string;
  user_subtitle: string;
}

export class HistoryController {
  private conversationHistory: Conversation[];
  private currentConversation: Conversation;
  private domUtils: typeof DomUtils;
  private ai_name: string;
  private user_name: string;

  constructor() {
    this.conversationHistory = [];
    this.currentConversation = {
      userMessage: "",
      aiResponses: [],
    };
    this.domUtils = DomUtils;
    this.ai_name = "默认";
    this.user_name = "默认";

    this.init();
  }

  private init(): void {
    this.bindEvents();
    this.render();
  }

  private bindEvents(): void {
    if (!DOM.history?.clearBtn || !DOM.history?.toggle) return;

    DOM.history.clearBtn.addEventListener(
      "click",
      this.handleClearHistory.bind(this)
    );

    DOM.history.toggle.addEventListener(
      "click",
      this.toggleHistoryPanel.bind(this)
    );

    EventBus.on("ui:name-updated", (names: Names) => {
      this.ai_name = names.ai_name;
      this.user_name = names.user_name;
      this.render();
    });
  }

  private handleClearHistory(): void {
    if (confirm("确定要清空所有历史记录吗？")) {
      this.conversationHistory = [];
      this.render();
    }
  }

  private toggleHistoryPanel(): void {
    if (!DOM.history?.toggle || !DOM.history?.content || !DOM.history?.clearBtn)
      return;

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

  private render(): void {
    if (!DOM.history?.list) return;

    DOM.history.list.innerHTML =
      this.conversationHistory.length === 0
        ? "<p>暂无历史记录</p>"
        : this.conversationHistory
            .map(
              (conv) => `
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

  public addMessage(
    userMessage: string | null,
    aiResponse: string | null,
    end: boolean = false
  ): void {
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

      this.currentConversation = { userMessage: "", aiResponses: [] };

      if (DOM.history?.content?.classList.contains("show")) {
        this.render();
      }
    }
  }
}

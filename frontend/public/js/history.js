const menuText = document.getElementById("menu-text");
const textPage = document.getElementById("text-page");

const menuImage = document.getElementById("menu-image");
const imagePage = document.getElementById("image-page") || document.getElementById("background-page");

const menuSound = document.getElementById("menu-sound");
const soundPage = document.getElementById("sound-page");

const historyToggle = document.getElementById("menu-history");
const historyContent = document.getElementById("history-page");
const historyList = document.getElementById("history-list");
const clearHistoryBtn = document.getElementById("clear-history");

// 对话历史数据
let conversationHistory = JSON.parse(localStorage.getItem("chatHistory")) || [];
let currentConversation = {
  userMessage: "",
  aiResponseParts: [],
};

// 初始化历史记录面板
function initHistoryPanel() {
  // 清空历史记录
  if (clearHistoryBtn) {
    clearHistoryBtn.addEventListener("click", () => {
      if (confirm("确定要清空所有历史记录吗？")) {
        conversationHistory = [];
        localStorage.setItem("chatHistory", JSON.stringify(conversationHistory));
        renderHistory();
      }
    });
  }

  // 切换显示/隐藏
  if (historyToggle) {
    historyToggle.addEventListener("click", () => {
      // 历史页面不使用动画
      if (historyToggle) historyToggle.classList.add("show");
      if (historyContent) historyContent.classList.add("show");
      if (clearHistoryBtn) clearHistoryBtn.classList.add("show");
      
      // 确保元素存在再操作它们
      if (menuText) menuText.classList.remove("show");
      if (textPage) textPage.classList.remove("show");
      if (menuImage) menuImage.classList.remove("show");
      if (imagePage) imagePage.classList.remove("show");
      if (menuSound) menuSound.classList.remove("show");
      if (soundPage) soundPage.classList.remove("show");
      
      renderHistory();
    });
  }
}

// 渲染历史记录
function renderHistory() {
  if (!historyList) return; // 如果historyList不存在，直接返回
  
  historyList.innerHTML = "";

  if (conversationHistory.length === 0) {
    historyList.innerHTML = "<p>暂无历史记录</p>";
    return;
  }

  conversationHistory.forEach((conversation, index) => {
    const item = document.createElement("div");
    item.className = "history-item";

    // 构建AI回复的HTML，每条回复单独显示
    let aiResponsesHTML = "";
    if (Array.isArray(conversation.aiResponses)) {
      conversation.aiResponses.forEach((response) => {
        aiResponsesHTML += `<p><strong>钦灵:</strong> ${response}</p>`;
      });
    } else {
      // 兼容旧数据（如果存在）
      aiResponsesHTML = `<p><strong>钦灵:</strong> ${
        conversation.aiResponse || ""
      }</p>`;
    }

    item.innerHTML = `
        <p><strong>你:</strong> ${conversation.userMessage}</p>
        ${aiResponsesHTML}
      `;

    historyList.appendChild(item);
  });
}

// 修改后的添加历史记录函数
function addToHistory(userMessage, aiResponse, end = false) {
  // 如果是新对话的开始
  if (userMessage && !currentConversation.userMessage) {
    currentConversation = {
      userMessage: userMessage,
      aiResponses: [], // 改为数组存储多条回复
    };
  }

  // 添加AI回复部分
  if (aiResponse) {
    currentConversation.aiResponses.push(aiResponse);
  }

  // 如果是对话结束，保存完整对话
  if (end) {
    if (
      currentConversation.userMessage &&
      currentConversation.aiResponses.length > 0
    ) {
      const conversation = {
        userMessage: currentConversation.userMessage,
        aiResponses: [...currentConversation.aiResponses], // 保存回复数组
      };

      conversationHistory.unshift(conversation);
      if (conversationHistory.length > 50) {
        conversationHistory.pop();
      }

      localStorage.setItem("chatHistory", JSON.stringify(conversationHistory));

      // 重置当前对话
      currentConversation = {
        userMessage: "",
        aiResponses: [],
      };

      // 如果有历史面板打开，更新显示
      if (!historyContent.classList.contains("hidden")) {
        renderHistory();
      }
    }
  }
}

// 在页面加载时初始化
initHistoryPanel();

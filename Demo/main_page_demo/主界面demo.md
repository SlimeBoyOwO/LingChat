# 主界面和设置界面 UI Demo 说明

## 1. 关于此 Demo

这是一个纯前端 UI Demo，可独立运行测试，与本项目其他文件互不干扰，旨在为游戏主界面设计和设置面板重构提供**视觉风格**与**交互设计**参考。

## 2. 实现功能概览



### by yukito

此 Demo 模拟了从游戏启动到进行详细设置的整个流程，包含以下主要功能模块：

* **加载动画（由喵哒子大人提供）**：

  * 一个可爱的猫耳主题加载动画，带有平滑的进度条和过渡效果。
  * 动画结束后自动淡出，并显示主界面。
* **主菜单**：

  * 清晰、简约的左侧垂直布局主菜单，包含"继续游戏"、"开始游戏"等核心选项。
  * 右上角展示 LingChat Logo。
* **全屏设置面板**：

  * 通过点击主菜单的"设置"按钮触发。
  * 以**全屏覆盖**的形式自下而上弹出，背景采用**毛玻璃效果**（`backdrop-filter`），与底层主界面内容形成视觉分离。
  * 右上角提供一个带悬停动画的关闭按钮。
* **导航系统**：

  * **顶部横向主导航**：位于设置面板顶部，图标与文字结合，直观展示"角色"、"文本"、"高级设置"等不同分类。
  * **滑动指示器**：当切换导航标签时，一个蓝色的指示器线段会平滑地移动到当前激活的标签下方，提供了明确的视觉反馈。
  * **动态内容切换**：点击不同导航项会立即显示对应的内容页面，无需重新加载。



### 7.9更新 by 风雪

### CSS: 以 `<body>` Class 作为状态机

我们通过在 `<body>` 上切换 Class 来控制整个应用的 UI 状态。

*   `body.panel-active`: 这是一个“基类”，表示“有某个面板是打开的”。它会触发：
    *   `main-container::before` 的 `filter: blur(12px) brightness(0.9)` 效果，即背景模糊变暗。
    *   `#start-page` 的 `transform: scale(0.98)` 缩小效果。
    *   `.main-menu` 和 `.logo` 的 `transform: translateX(...)` 滑出效果。

*   `body.show-*` (例如 `show-settings`, `show-game-screen`): 这是一个“状态类”，用于精确显示**哪一个**面板。
    *   例如，当 `body.show-settings` 存在时，`#settings-panel` 的 `transform` 会变为 `translateX(0)`，使其滑入视图。

JavaScript 只需要负责在 `<body>` 上增删 Class，所有动画和显隐逻辑都由 CSS 声明式地处理，非常清晰。

### CSS: 背景处理 (`.main-container::before`)

为了实现背景模糊而前景不模糊，背景图从 `.main-container` 转移到了它的伪元素 `.main-container::before` 上。这样，可以独立地对伪元素应用 `filter: blur()`，而不会影响到容器内的所有子元素（如主菜单、Logo）。

### JavaScript: 通用面板控制器 (`main.js`)

`main.js` 的新逻辑非常简单：

*   `activePanelId`：一个全局变量，用于追踪当前打开的面板 ID。
*   `openPanel(panelId)`:
    1.  接收要打开的面板 ID。
    2.  给 `<body>` 添加 `panel-active` 和对应的 `show-*` Class。
    3.  更新 `activePanelId`。
    4.  （首次打开设置面板时）执行一次性的初始化函数 `setupSettingsInteraction()`。
*   `closePanel()`:
    1.  移除 `<body>` 上的所有面板相关 Class。
    2.  重置 `activePanelId` 为 `null`。

所有菜单按钮的点击事件都统一调用 `openPanel()`，所有关闭按钮和“点击外部”的逻辑都统一调用 `closePanel()`。

*   **如何添加一个新面板？**
    1.  **HTML (`index.html`)**: 在 `.main-container` 内仿照 `settings-panel` 添加你的新面板HTML结构，给它一个唯一的 `id` (例如 `id="new-feature-panel"`)。记得在面板的关闭按钮上添加 `data-close-panel="new-feature-panel"` 属性。
    2.  **CSS (`style.css`)**: 在 `/* 特定面板的显示规则 */` 部分，添加一个新的规则：`body.panel-active.show-new-feature .new-feature-panel { transform: translateX(0); opacity: 1; }`。你可以创建新的 CSS 文件来编写面板内部样式。
    3.  **JavaScript (`main.js`)**: 找到触发这个面板的按钮，给它绑定一个 `click` 事件，调用 `openPanel('new-feature-panel')`。JS 需要知道你的 `panelId` 和 `show-*` 类的对应关系，请在 `openPanel` 和 `closePanel` 函数中按需添加 `if/else` 分支。
*   **关于资源路径**：
    *   请注意，Demo 中的图片路径（如 `../../frontend/public/...`）是基于当前文件位置的相对路径。在实际项目中肯定要改的。

## 3. 设计与资源

*   **字体**: Demo 主界面使用的主要字体为 **猫啃什锦黑**，可免费商用，允许嵌入。
    *   **字体来源**: [猫啃网](https://www.maoken.com/assorted)

## 4. 重要声明

- **仅为UI演示**：此 Demo 是一个独立的、用于展示视觉和交互的**样式参考**，其代码**不会被直接合并或整合**到 `LingChat` 的主项目中。
- **未来架构待定**：`LingChat` 项目的 UI 将如何重构，以及是否会采用如 `Vue`、`React` 等现代化前端框架，**目前仍有待讨论和确认**。

## 5. 如何本地运行

1. 在**文件资源管理器**中，导航到 `LingChat/frontend/public/main_page_demo/` 目录。
2. 双击 `index.html` 文件，它将在默认浏览器中打开。

## 6. 运行截图

![image](https://github.com/user-attachments/assets/bedcbc49-700f-4eff-90c6-e4807c8d08c4)

![image](https://github.com/user-attachments/assets/72630e8c-17f8-41fd-8eb0-174aeab928ab)

![image](https://github.com/user-attachments/assets/b4e8d099-c0c3-4818-90aa-83ed0eabee2d)


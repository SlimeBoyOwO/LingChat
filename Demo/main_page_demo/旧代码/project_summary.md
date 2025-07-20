# main_page_demo 项目概览

## 项目结构
```text
main_page_demo/
├── css/
│   ├── components/
│   │   ├── main_menu.css
│   │   ├── page_advanced.css
│   │   ├── page_character.css
│   │   ├── page_text.css
│   │   └── settings_panel.css
│   ├── loader.css
│   └── style.css
├── fonts/
│   └── MaokenAssortedSans.ttf
├── js/
│   ├── loader.js
│   └── main.js
├── index.html
└── 主界面demo.md
```

## 文件内容
### 文件: `css/components/main_menu.css`

```css
/* 主容器，用于设置背景和布局 */
.main-container {
    width: 100%;
    height: 100%;
    background-image: url('../../../../frontend/public/pictures/backgrounds/login/X0_6rTZl.png');
    background-size: cover;
    background-position: center;
    display: flex;
    justify-content: flex-start; /* 将主菜单推到左边 */
    align-items: center;
    position: relative; /* 为绝对定位的logo提供容器 */
}

/* 主菜单 */
.main-menu {
    display: flex;
    flex-direction: column;
    /* align-items: center; */
    padding: 20px;
    margin-left: 10vw; /* 距离左侧边缘10%视口宽度 */
}

.logo {
    position: absolute;
    top: 5vh; /* 距离顶部5%视口高度 */
    right: 5vw; /* 距离右侧5%视口宽度 */
    height: 40vh; /* 高度为视口高度的40% */
    width: auto; /* 宽度自动，保持比例 */
    max-width: 40vw; /* 最大宽度不超过视口宽度的40%，防止过宽 */
    filter: drop-shadow(0 4px 8px rgba(0,0,0,0.3)); /* 添加一点阴影使其更突出 */
}

.menu-options {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    width: 350px; /* 按钮加宽一点 */
}

.menu-button {
    background: transparent; /* 去除背景 */
    color: white;
    border: none; /* 去除边框 */
    padding: 15px;
    margin: 10px 0;
    border-radius: 12px;
    /* 使用clamp()实现响应式字体大小 */
    /* 最小32px, 根据视口宽度的4%缩放, 最大72px */
    font-size: clamp(32px, 4vw, 72px);
    font-weight: normal; /* 字体加粗 */
    font-family: 'Maoken Assorted Sans', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; /* 应用自定义字体，并提供备用字体 */
    cursor: pointer;
    transition: color 0.3s, text-shadow 0.3s; /* 平滑过渡 */
    text-shadow: 0 2px 4px rgba(0,0,0,0.5); /* 加一点文字阴影以保证清晰度 */
    text-align: left; /* 文字左对齐 */
}

.menu-button:hover {
    color: #f0f0f0;
    text-shadow: 0 0 10px rgba(255, 255, 255, 0.8); /* 悬停时发光效果 */
    transform: none; /* 移除之前的缩放效果 */
}
```

### 文件: `css/components/page_advanced.css`

```css
/* --- 高级设置页面样式 --- */
.advanced-settings-container header {
    padding-bottom: 15px;
    margin-bottom: 25px;
}

.advanced-settings-container .adv-title {
    margin: 0;
    font-size: 24px;
    color: var(--accent-color);
    font-weight: 600;
    position: relative; /* 为指示器提供定位上下文 */
}

.advanced-settings-container .adv-description {
    margin: 8px 0 0;
    color: #555;
    font-size: 16px;
}

.settings-form {
    max-width: 800px; /* 限制最大宽度，在宽屏上更美观 */
}

.form-group {
    margin-bottom: 24px;
}

.form-group label {
    display: block;
    font-weight: bold;
    margin-bottom: 6px;
    color: #333;
}

.form-group .description {
    font-size: 13px;
    color: #777;
    margin-bottom: 8px;
    display: block;
}

.form-group .form-control {
    width: 100%;
    padding: 10px 12px;
    border: 1px solid #ddd;
    background-color: #fff;
    border-radius: 8px;
    color: #333;
    font-size: 15px;
    font-family: inherit;
    transition: border-color 0.2s, box-shadow 0.2s;
}

.form-group .form-control:focus {
    outline: none;
    border-color: var(--accent-color);
    box-shadow: 0 0 0 3px rgba(121, 217, 255, 0.3); /* 使用主题色光晕作为 a11y focus */
}

.form-group input[type="password"] {
    font-family: 'Courier New', Courier, monospace; /* 让密码更易读 */
}

/* --- 高级设置页面基础布局 --- */
.advanced-settings-grid {
    display: grid;
    grid-template-columns: 280px 1fr; /* 侧边栏固定宽度，内容区自适应 */
    height: 100%;
}

/* --- 高级设置侧边导航栏 --- */
.advanced-nav {
    background-color: #f0f4f8; /* 一个非常浅的蓝色，与背景区分 */
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 25px;
    overflow-y: auto; /* 当导航项过多时，使其可以独立滚动 */
    position: relative; /* 为指示器提供定位上下文 */
}

/* 新增：二级导航滑动指示器 */
.adv-nav-indicator {
    position: absolute;
    top: 0; /* JS会更新 */
    left: 20px; /* 左右留出一些边距 */
    width: calc(100% - 40px); /* 左右留出一些边距 */
    height: 0; /* JS会更新 */
    background-color: var(--accent-color);
    border-radius: 6px;
    z-index: 0; /* 确保在链接文字下方 */
    transition: top 0.3s ease-in-out, height 0.3s ease-in-out;
}

/* 新增：临时禁用过渡动画的类 */
.adv-nav-indicator.no-transition {
    transition: none;
}

.advanced-nav .adv-nav-title {
    font-size: 22px;
    color: #333;
    margin: 10px 0;
    text-align: center;
    font-weight: 600;
    border-radius: 6px;
    transition: background-color 0.2s, color 0.2s;
    border-left: 4px solid transparent;
    position: relative; /* 确保文字在指示器上方 */
    z-index: 1; /* 确保文字在指示器上方 */
}

.advanced-nav .adv-nav-category {
    display: flex;
    flex-direction: column;
    gap: 5px;
}

.advanced-nav .category-title {
    font-size: 16px;
    font-weight: bold;
    color: #fff;
    padding: 10px 15px;
    display: block;
    background-color: #a0b1c9; /* 一个柔和的蓝色作为分类标题背景 */
    border-radius: 8px;
    margin-bottom: 5px;
}

.advanced-nav .adv-nav-link {
    display: block;
    padding: 12px 20px;
    color: #374151;
    text-decoration: none;
    border-radius: 6px;
    transition: background-color 0.2s, color 0.2s;
    border-left: 4px solid transparent;
    position: relative; /* 确保文字在指示器上方 */
    z-index: 1; /* 确保文字在指示器上方 */
}

.advanced-nav .adv-nav-link:hover:not(.active) {
    background-color: #e5e7eb;
}

.advanced-nav .adv-nav-link.active {
    color: white;
    font-weight: bold;
}

/* --- 高级设置内容区 --- */
.advanced-content {
    padding: 0 40px;
    overflow-y: auto;
    display: flex; /* 使用 Flexbox 布局 */
    justify-content: center; /* 水平居中内容 */
}

.adv-content-page {
    display: none;
    width: 100%; /* 确保页面容器占满可用宽度 */
    max-width: 900px; /* 设定一个最大宽度，避免在超宽屏上过分拉伸 */
}

.adv-content-page.active {
    display: block;
}

/* 原有的表单样式保持不变，但为了确保它们在内容区内正确显示，检查一下 */
.advanced-settings-container {
    padding-top: 20px;
}

.adv-content-page .settings-form .form-group .description {
    font-size: 0.8rem;
    color: #666;
    margin-top: 4px;
    margin-bottom: 8px;
}

/* Textarea Style */
.adv-content-page .settings-form .form-group textarea.form-control {
    width: 100%;
    padding: 10px;
    border: 1px solid #ccc;
    border-radius: 4px;
    font-size: 0.9rem;
    background-color: #fff;
    color: #333;
    transition: border-color 0.3s;
    min-height: 120px;
    resize: vertical;
}

.adv-content-page .settings-form .form-group textarea.form-control:focus {
    outline: none;
    border-color: var(--accent-color);
}


/* Checkbox Style */
.adv-content-page .settings-form .form-group .checkbox-label {
    display: flex;
    align-items: center;
    cursor: pointer;
    font-weight: 500;
}

.adv-content-page .settings-form .form-group .checkbox-label input[type="checkbox"] {
    margin-right: 10px;
    width: 16px;
    height: 16px;
    accent-color: var(--accent-color);
}
```

### 文件: `css/components/page_character.css`

```css
#character-settings.active {
    display: flex;
    flex-direction: column;
    align-items: center; /* 水平居中所有子元素 */
}

#character-settings {
    color: #333;
    padding: 0 20px; /* 增加一些左右内边距 */
}

#character-settings > .section-header,
#character-settings > .character-grid,
#character-settings > .actions-footer {
    width: 100%;
    max-width: 900px; /* 与高级设置保持一致的最大宽度 */
}

/* --- 角色选择页面新样式 --- */

.section-header {
    display: flex;
    align-items: center;
    border-bottom: 2px solid var(--accent-color);
    padding-bottom: 8px;
    margin-bottom: 15px;
}

.section-header h4, .section-header h5 {
    margin: 0;
    font-size: 18px;
    color: #333;
    font-weight: 600;
}

.character-grid {
    display: grid;
    grid-template-columns: 1fr; /* 从多列网格改为单列 */
    gap: 20px;
    margin-bottom: 30px;
}

.character-card {
    display: flex;
    background-color: #fff;
    border-radius: 12px;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.05);
    overflow: hidden;
    transition: all 0.3s ease;
}

.character-card:hover {
    transform: translateY(-5px);
    box-shadow: 0 8px 25px rgba(0, 0, 0, 0.1);
}

.character-avatar-container {
    width: 150px;
    height: 100%;
    flex-shrink: 0;
    padding: 15px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, #f5f7fa 0%, #e8ecf3 100%);
}

.character-avatar {
    width: 100%;
    height: auto;
    object-fit: contain;
    border-radius: 8px;
}

.character-content-wrapper {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 15px;
    position: relative;
}

.character-title {
    font-size: 16px;
    font-weight: 700;
    color: #333;
    margin-top: 0;
    margin-bottom: 8px;
}

.character-description {
    font-size: 13px;
    color: #666;
    line-height: 1.5;
    flex-grow: 1;
    margin-bottom: 15px;
}

.character-select-btn {
    align-self: flex-end; /* 按钮靠右 */
    background-color: #ccc;
    color: #666;
    border: none;
    padding: 8px 15px;
    border-radius: 20px;
    cursor: pointer;
    transition: all 0.2s ease;
    font-size: 13px;
    font-weight: 500;
}
.character-select-btn.selected {
    background-color: var(--accent-color);
    color: white;
}
.character-select-btn:not(.selected):hover {
    background-color: #555;
    color: white;
}

.actions-footer {
    display: grid;
    grid-template-columns: 1fr 1fr; /* 恢复为双列布局，让两个按钮并排显示 */
    gap: 20px;
    width: 100%;
}

.action-button {
    width: 100%;
    padding: 12px;
    font-size: 16px;
    font-weight: bold;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    background-color: #e9ecef;
    color: #495057;
    transition: all 0.2s ease;
}

.action-button:hover {
    background-color: var(--accent-color);
    color: white;
    transform: translateY(-2px);
    box-shadow: 0 4px 10px rgba(121, 217, 255, 0.4);
}
```

### 文件: `css/components/page_text.css`

```css
.action-button.full-width {
    width: 100%;
}

/* --- 文本设置页面新样式 --- */
.settings-columns {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0px;
    height: 100%;
}

.setting-item {
    margin-bottom: 25px;
    width: 100%;
    max-width: 900px;
}

/* 滑块样式 (移植自 menu.css) */
.slider-container {
    display: flex;
    align-items: center;
    gap: 15px;
    color: #666;
}

input[type="range"] {
    -webkit-appearance: none;
    appearance: none;
    flex-grow: 1;
    margin: 10px 0;
    background-color: transparent;
    position: relative;
    outline: none;
}

input[type="range"]::-webkit-slider-runnable-track {
    width: 100%;
    height: 8px;
    border-radius: 4px;
    background: linear-gradient(to left, var(--accent-color), #c4eeff); /* 应用主题色渐变 */
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.1);
    transition: all 0.3s ease;
}

input[type="range"]:hover::-webkit-slider-runnable-track {
    height: 10px;
}

input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 22px;
    height: 22px;
    background: #ffffff;
    border-radius: 50%;
    cursor: grab;
    border: 2px solid var(--accent-color); /* 应用主题色 */
    margin-top: -7px;
    transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
    transform-origin: center;
    box-shadow: 0 2px 8px rgba(121, 217, 255, 0.3); /* 应用主题色阴影 */
    position: relative;
    z-index: 2;
}

input[type="range"]::-webkit-slider-thumb:hover,
input[type="range"]::-webkit-slider-thumb:active {
    transform: scale(1.15);
    box-shadow: 0 0 12px rgba(121, 217, 255, 0.5); /* 应用主题色阴影 */
}

input[type="range"]:active::-webkit-slider-thumb {
    cursor: grabbing;
}

/* 开关样式 */
.toggle-container {
    display: flex;
    align-items: center;
}

.toggle-input {
    display: none;
}

.toggle-label {
    cursor: pointer;
    position: relative;
    padding-left: 60px;
    font-size: 14px;
    color: #333;
}

.toggle-label::before {
    content: '';
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 50px;
    height: 26px;
    background-color: #e9ecef;
    border-radius: 13px;
    transition: background-color 0.3s;
}

.toggle-label::after {
    content: '';
    position: absolute;
    left: 4px;
    top: 50%;
    transform: translateY(-50%);
    width: 20px;
    height: 20px;
    background-color: white;
    border-radius: 50%;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    transition: left 0.3s;
}

.toggle-input:checked + .toggle-label::before {
    background-color: var(--accent-color);
}

.toggle-input:checked + .toggle-label::after {
    left: 26px;
}

/* 文本样本显示区域 */
.text-sample-display {
    background-color: #343a40;
    color: #f8f9fa;
    padding: 15px 20px;
    border-radius: 8px;
    font-family: 'Courier New', Courier, monospace;
    min-height: 2.5em; /* 确保有足够高度容纳光标 */
}

/* 打字光标动画 */
.typing-cursor {
    display: inline-block;
    height: 1.2em;
    width: 3px;
    vertical-align: text-bottom;
    margin-left: 4px;
    background-color: var(--accent-color);
    animation: cursor-blink 0.8s infinite;
}

@keyframes cursor-blink {
    0%, 100% {
        opacity: 1;
    }
    50% {
        opacity: 0;
    }
}
```

### 文件: `css/components/settings_panel.css`

```css
/* --- 设置面板新样式 (v2) --- */
.settings-panel {
    position: fixed;
    top: 0; /* 从顶部开始定位 */
    left: 0;
    width: 100%;
    height: 100%; /* 高度占满全屏 */
    transform: translateY(100%);
    transition: transform 0.5s cubic-bezier(0.25, 0.46, 0.45, 0.94);
    padding: 0;
    box-sizing: border-box;
    z-index: 1000;
    background: rgba(253, 253, 253, 0.8); /* 使用带alpha通道的颜色以实现半透明 */
    color: #333;
    /* 添加毛玻璃效果 */
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
}

.settings-panel.active {
    transform: translateY(0);
}

.settings-header {
    display: flex;
    align-items: center;
    padding: 10px 15px; /* 调整内边距 */
    /* border-bottom: 1px solid #eee; */ /* 移除分割线 */
    position: relative;
    justify-content: space-between; /* 将两端元素推开 */
}

.settings-logo {
    width: 80px; /* logo稍微再小一点 */
    height: auto;
    margin-right: 0;
}

.settings-header h2 { /* 此选择器暂时无用，但保留 */
    margin: 0;
    color: #333;
    text-shadow: none;
    font-size: 24px;
    flex-grow: 1;
    transition: color 0.2s;
    margin-left: 0; /* 移除 auto margin */
}

.close-button {
    /* 重置按钮样式 */
    background: transparent;
    border: none;
    padding: 6px; /* 为背景光晕留出空间 */
    border-radius: 50%; /* 圆形按钮 */
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #aaa; /* 图标初始颜色 */
    transition: all 0.3s ease-in-out;
}
.close-button:hover {
    color: var(--accent-color);
    background-color: rgba(121, 217, 255, 0.1); /* 主题色光晕 */
    transform: rotate(90deg); /* 旋转动画 */
}

.close-button svg {
    width: 28px;
    height: 28px;
}

.settings-body {
    display: flex;
    height: calc(100% - 68px); /* 重新计算高度 */
    margin-top: 0;
}

.settings-nav {
    position: absolute; /* 使用绝对定位脱离文档流 */
    left: 50%; /* 定位到中心 */
    transform: translateX(-50%); /* 向左移动自身宽度的一半，实现完美居中 */

    display: flex;
    flex-direction: row; /* 水平排列 */
    padding: 0;
    border-right: none; /* 移除右边框 */
    height: 100%; /* 占满header的高度 */
    align-items: center; /* 垂直居中按钮 */
}

/* 滑动指示器 */
.nav-indicator {
    position: absolute;
    bottom: 0; /* 贴在导航栏底部 */
    left: 0; /* JS会更新 */
    width: 0; /* JS会更新 */
    height: 4px; /* 指示器变为一条线 */
    background-color: var(--accent-color);
    border-radius: 2px;
    z-index: 1;
    transition: left 0.3s ease-in-out, width 0.3s ease-in-out;
}

.settings-nav .nav-button {
    background: none;
    color: #555;
    text-align: center;
    padding: 10px 15px; /* 调整按钮内边距 */
    border-radius: 8px; /* 加回一点圆角 */
    border: none;
    cursor: pointer;
    margin: 0 5px; /* 按钮间距 */
    font-size: 16px;
    font-weight: bold;
    position: relative;
    z-index: 1;
    transition: color 0.3s ease, background-color 0.3s ease;
    /* 新增flex布局来对齐图标和文字 */
    display: flex;
    align-items: center;
    gap: 8px; /* 图标和文字的间距 */
}

.settings-nav .nav-button svg {
    width: 18px; /* 设置图标大小 */
    height: 18px;
    stroke-width: 2.5px; /* 图标线条粗细 */
    flex-shrink: 0; /* 防止图标被压缩 */
}

.settings-nav .nav-button:hover {
    color: var(--accent-color); /* 悬停时只改变文字颜色 */
}

.settings-nav .nav-button.active {
    color: var(--accent-color); /* 激活时文字变主题色 */
    background-color: transparent;
}
.settings-nav .nav-button.active:hover {
    color: var(--accent-color);
    background-color: transparent; /* 确保激活状态下悬停也没有背景 */
}

.settings-content {
    flex-grow: 1;
    background: none;
    border-radius: 0;
    padding: 20px;
}

/* 这是一个通用页面样式，可以放在这里 */
.settings-page {
    display: none; /* 默认隐藏所有页面 */
    color: #333;
    height: 100%; /* 让每个页面都撑满父容器的高度 */
}

.settings-page.active {
    display: block; /* 只显示带有 active 类的页面 */
}
```

### 文件: `css/loader.css`

```css
@charset "utf-8";

body.loading-active {
    overflow: hidden;
}

#loader {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    -webkit-backdrop-filter: blur(16px);
    backdrop-filter: blur(16px);
    z-index: 9999;
    opacity: 1;
    transition: opacity 0.8s ease-out, backdrop-filter 0.25s, opacity 0.35s;
}

#loader::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    z-index: -1;
    pointer-events: none;
    background-image: linear-gradient(135deg, rgb(224, 231, 255, 1), rgb(199, 210, 254, 1));
    opacity: 1;
    transition: opacity 0.35s;
}

#loader.bg-transparent::before {
    opacity: 0;
    transition: opacity 0.35s;
}

#loader.no-blur {
    -webkit-backdrop-filter: none;
    backdrop-filter: none;
    transition: backdrop-filter 0.25s, -webkit-backdrop-filter 0.25s, opacity 0.35s;
}

#loader.hidden {
    opacity: 0;
    pointer-events: none;
}

.ears-container {
    position: relative;
    width: 150px;
    height: 80px;
    animation: bounce 2.5s infinite ease-in-out;
}

.progress-bar-container {
    width: 180px;
    height: 10px;
    background: #e0e7ff;
    border-radius: 5px;
    margin: 24px auto 0 auto;
    box-shadow: 0 2px 8px rgba(67, 56, 202, 0.08);
    overflow: hidden;
}

.progress-bar {
    height: 100%;
    width: 0;
    background: linear-gradient(90deg, #818cf8, #fbcfe8);
    border-radius: 5px;
    transition: width 0.4s cubic-bezier(.4,2,.6,1), background 0.5s;
    will-change: width, background;
}

@keyframes bounce {

    0%,
    100% {
        transform: translateY(0);
    }

    50% {
        transform: translateY(-10px);
    }
}

.ear {
    position: absolute;
    bottom: 0;
    width: 55px;
    height: 80px;
    background-color: #fff;
    border: 3px solid #a5b4fc;
    border-radius: 51% 49% 45% 55% / 100% 100% 0% 0%;
    box-shadow: 0 4px 15px rgba(165, 180, 252, 0.3);
}

.ear::before {
    content: "";
    position: absolute;
    top: 21px;
    left: 50%;
    transform: translateX(-50%);
    width: 25px;
    height: 50px;
    background-color: #fbcfe8;
    border-radius: 51% 49% 45% 55% / 100% 100% 0% 0%;
}

.ear-left {
    left: 10px;
    transform-origin: bottom center;
    transform: rotate(-15deg);
    animation: wag-left 2.5s infinite ease-in-out;
}

.ear-right {
    right: 10px;
    transform-origin: bottom center;
    transform: rotate(15deg);
    animation: wag-right 2.5s infinite ease-in-out;
}

@keyframes wag-left {

    0%,
    100% {
        transform: rotate(-15deg);
    }

    50% {
        transform: rotate(-25deg);
    }
}

@keyframes wag-right {

    0%,
    100% {
        transform: rotate(15deg);
    }

    50% {
        transform: rotate(25deg);
    }
}

.loading-text {
    margin-top: 30px;
    font-size: 1.2em;
    color: #4338ca;
    font-weight: 500;
    text-align: center;
    font-family: 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', Arial, sans-serif;
}

.loading-text::after {
    content: ".";
    animation: dots 1.4s infinite;
}

@keyframes dots {

    0%,
    20% {
        content: ".";
    }

    40% {
        content: "..";
    }

    60%,
    100% {
        content: "...";
    }
}

@media (max-width: 600px) {
    .ears-container {
        width: 90px;
        height: 48px;
    }

    .ear {
        width: 32px;
        height: 48px;
    }

    .ear::before {
        width: 14px;
        height: 28px;
        top: 12px;
    }

    .loading-text {
        font-size: 1em;
    }
}
```

### 文件: `css/style.css`

```css
/* 定义自定义字体 */
@font-face {
    font-family: 'Maoken Assorted Sans';
    src: url('../fonts/MaokenAssortedSans.ttf') format('truetype');
    font-weight: normal;
    font-style: normal;
}

:root {
    --accent-color: #79d9ff; /* 从Logo提取的主题粉色 */
}

/* 全局样式和字体 */
body, html {
    margin: 0;
    padding: 0;
    width: 100%;
    height: 100%;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    overflow: hidden;
}
```

### 文件: `index.html`

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>LingChat - 主界面Demo</title>
    <link rel="icon" href="../../frontend/public/pictures/icons/小猫Ling.svg" type="image/svg+xml">
    <link rel="stylesheet" href="css/style.css">
    <link rel="stylesheet" href="css/loader.css">
    <!-- Component Styles -->
    <link rel="stylesheet" href="css/components/main_menu.css">
    <link rel="stylesheet" href="css/components/settings_panel.css">
    <link rel="stylesheet" href="css/components/page_character.css">
    <link rel="stylesheet" href="css/components/page_text.css">
    <link rel="stylesheet" href="css/components/page_advanced.css">
</head>
<body>
    <div id="loader">
        <div class="ears-container">
            <div class="ear ear-left"></div>
            <div class="ear ear-right"></div>
        </div>
        <div class="progress-bar-container">
            <div class="progress-bar" id="progress-bar"></div>
        </div>
        <div class="loading-text">你的小可爱正在准备中</div>
    </div>

    <div class="main-container">
        <img src="../../frontend/public/pictures/Image/LingChatLogo.png" alt="LingChat Logo" class="logo">
        <!-- 主菜单 -->
        <div class="main-menu">
            <nav class="menu-options">
                <button class="menu-button">继续游戏</button>
                <button class="menu-button">开始游戏</button>
                <button class="menu-button">存&emsp;&emsp;档</button>
                <button class="menu-button" id="settings-button">设&emsp;&emsp;置</button>
                <button class="menu-button">退出游戏</button>
            </nav>
        </div>

        <!-- 设置面板 (默认隐藏) -->
        <div class="settings-panel" id="settings-panel">
            <div class="settings-header">
                <img src="../../frontend/public/pictures/Image/LingChatLogo.png" alt="Logo" class="settings-logo">
                <nav class="settings-nav">
                    <div class="nav-indicator"></div>
                    <button class="nav-button active" data-content="character-settings">
                        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 20a6 6 0 0 0-12 0"/><circle cx="12" cy="10" r="4"/><circle cx="12" cy="12" r="10"/></svg>
                        <span>角色</span>
                    </button>
                    <button class="nav-button" data-content="text-settings">
                        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="21" y1="10" x2="3" y2="10"/><line x1="21" y1="6" x2="3" y2="6"/><line x1="21" y1="14" x2="3" y2="14"/><line x1="21" y1="18" x2="3" y2="18"/></svg>
                        <span>文本</span>
                    </button>
                    <button class="nav-button" data-content="background-settings">
                        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/></svg>
                        <span>背景</span>
                    </button>
                    <button class="nav-button" data-content="sound-settings">
                        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 10v3M6 7v9M10 4v15M14 7v9M18 10v3"/></svg>
                        <span>声音</span>
                    </button>
                    <button class="nav-button" data-content="history-settings">
                        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="20" height="5" x="2" y="4" rx="2"/><path d="M4 9v9a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9"/><path d="M10 13h4"/></svg>
                        <span>对话历史</span>
                    </button>
                    <button class="nav-button" data-content="advanced-settings">
                        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="4" x2="4" y1="21" y2="14" /><line x1="4" x2="4" y1="10" y2="3" /><line x1="12" x2="12" y1="21" y2="12" /><line x1="12" x2="12" y1="8" y2="3" /><line x1="20" x2="20" y1="21" y2="16" /><line x1="20"x2="20" y1="12" y2="3" /><line x1="2" x2="6" y1="14" y2="14" /><line x1="10" x2="14" y1="8" y2="8" /><line x1="18" x2="22" y1="16" y2="16" /></svg>
                        <span>高级设置</span>
                    </button>
                </nav>
                <button class="close-button" id="close-settings-button" title="关闭">
                    <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                </button>
            </div>
            <div class="settings-body">
                <main class="settings-content">
                    <div class="settings-page active" id="character-settings">
                        
                        <div class="section-header">
                            <i class="icon-character"></i>
                            <h4>角色列表</h4>
                        </div>
                        <div class="character-grid">
                            <div class="character-card">
                                <div class="character-avatar-container">
                                    <img src="../../game_data/characters/诺一钦灵/avatar/正常.png" alt="可爱的小狼娘" class="character-avatar">
                                </div>
                                <div class="character-content-wrapper">
                                    <h5 class="character-title">可爱的小狼娘</h5>
                                    <p class="character-description">这是一只可爱的小狼娘。喜欢粘你，做娇...又可爱的小程序员。喜欢变态的玩法，是你的好助手~</p>
                                    <button class="character-select-btn selected">✓ 已选择</button>
                                </div>
                            </div>
                            <!-- 更多角色卡片可以加在这里 -->
                        </div>

                        <div class="actions-footer">
                            <div class="action-section">
                                <div class="section-header">
                                    <i class="icon-refresh"></i>
                                    <h5>刷新人物列表</h5>
                                </div>
                                <button class="action-button">点我刷新~</button>
                            </div>
                            <div class="action-section">
                                <div class="section-header">
                                    <i class="icon-workshop"></i>
                                    <h5>创意工坊</h5>
                                </div>
                                <button class="action-button">进入创意工坊</button>
                            </div>
                        </div>

                    </div>
                    <!-- 其他设置页面可以先放在这里，默认隐藏 -->
                    <div class="settings-page" id="text-settings">
                        <div class="settings-columns">
                            <!-- 文字显示速度 -->
                            <div class="setting-item">
                                <div class="section-header">
                                    <h4>文字显示速度</h4>
                                </div>
                                <div class="slider-container">
                                    <span>慢</span>
                                    <input type="range" id="text-speed-slider" min="1" max="100" value="50" aria-label="文字显示速度">
                                    <span>快</span>
                                </div>
                            </div>

                            <!-- 显示样本 -->
                            <div class="setting-item">
                                <div class="section-header">
                                    <h4>显示样本</h4>
                                </div>
                                <div class="text-sample-display">
                                    <span id="typed-text-sample"></span><span class="typing-cursor"></span>
                                </div>
                            </div>
            
                            <!-- 页面切换动画 -->
                            <div class="setting-item">
                                <div class="section-header">
                                    <h4>页面切换动画</h4>
                                </div>
                                <div class="toggle-container">
                                    <input type="checkbox" id="animation-toggle" class="toggle-input" checked>
                                    <label for="animation-toggle" class="toggle-label">启用动画</label>
                                </div>
                            </div>
            
                            <!-- 无语音音效开关 -->
                            <div class="setting-item">
                                <div class="section-header">
                                    <h4>无语音音效开关</h4>
                                </div>
                                <div class="toggle-container">
                                    <input type="checkbox" id="sound-effect-toggle" class="toggle-input" checked>
                                    <label for="sound-effect-toggle" class="toggle-label">启用无vits时的对话音效</label>
                                </div>
                            </div>
                        </div>
                    </div>
                    <div class="settings-page" id="background-settings"><p>背景设置</p></div>
                    <div class="settings-page" id="sound-settings"><p>声音设置</p></div>
                    <div class="settings-page" id="history-settings"><p>对话历史</p></div>
                    <div class="settings-page" id="advanced-settings">
                        
                        <div class="advanced-settings-grid">
                            <nav class="advanced-nav">
                                <div class="adv-nav-indicator"></div>
                                <div class="adv-nav-category">
                                    <span class="category-title">基础设置</span>
                                    <a href="#" class="adv-nav-link active" data-content="api-model-settings">API 与 模型 设置</a>
                                    <a href="#" class="adv-nav-link" data-content="dialogue-settings">对话功能设定</a>
                                    <a href="#" class="adv-nav-link" data-content="character-config-settings">人物设定 (弃用)</a>
                                </div>
                                <div class="adv-nav-category">
                                    <span class="category-title">开发者设置</span>
                                    <a href="#" class="adv-nav-link" data-content="rag-settings">RAG系统设定</a>
                                    <a href="#" class="adv-nav-link" data-content="storage-log-settings">存储与日志</a>
                                    <a href="#" class="adv-nav-link" data-content="debug-settings">Debug信息</a>
                                    <a href="#" class="adv-nav-link" data-content="port-settings">服务端口配置</a>
                                    <a href="#" class="adv-nav-link" data-content="vits-settings">VITS语音与模型</a>
                                </div>
                            </nav>
                            <main class="advanced-content">
                                <!-- API与模型设置 -->
                                <div class="adv-content-page active" id="api-model-settings">
                                    <div class="advanced-settings-container">
                                        <header>
                                            <h4 class="adv-title">API 与 模型 设置</h4>
                                            <p class="adv-description">配置与AI模型和API相关的密钥和地址</p>
                                        </header>
                                        <div class="settings-form">
                                            <div class="form-group">
                                                <label for="llm-provider">LLM_PROVIDER</label>
                                                <small class="description">在这里选择对话模型, 只可以从后weblm, ollama, lmstudio, gemini四个选项, weblm代表通用需要联网的AI模型 (如deepseek) , ollama和lmstudio表示本地)</small>
                                                <input type="text" id="llm-provider" class="form-control" value="weblm">
                                            </div>
                                            <div class="form-group">
                                                <label for="chat-api-key">CHAT_API_KEY</label>
                                                <small class="description">DeepSeek 或其他聊天模型的 API Key</small>
                                                <input type="password" id="chat-api-key" class="form-control" value="sk-6b6d2bd24e554c3f9142b47cbecc460a">
                                            </div>
                                             <div class="form-group">
                                                <label for="vd-api-key">VD_API_KEY</label>
                                                <small class="description">图片识别模型的 API Key</small>
                                                <input type="password" id="vd-api-key" class="form-control" value="sk-1b8eb3a19bfd48ebb0320e3bfadc8e95">
                                            </div>
                                            <div class="form-group">
                                                <label for="chat-base-url">CHAT_BASE_URL</label>
                                                <small class="description">API的访问地址</small>
                                                <input type="text" id="chat-base-url" class="form-control" value="https://api.deepseek.com">
                                            </div>
                                            <div class="form-group">
                                                <label for="model-type">MODEL_TYPE</label>
                                                <small class="description">使用的模型类型</small>
                                                <input type="text" id="model-type" class="form-control" value="deepseek-chat">
                                            </div>
                                            <div class="form-group">
                                                <label for="vd-base-url">VD_BASE_URL</label>
                                                <small class="description">视觉模型的API访问地址</small>
                                                <input type="text" id="vd-base-url" class="form-control" value="https://dashscope.aliyuncs.com/compatible-mode/v1">
                                            </div>
                                           <div class="form-group">
                                                <label for="VD_MODEL">VD_MODEL</label>
                                                <small class="description">视觉模型的模型类型</small>
                                                <input type="text" id="VD_MODEL" class="form-control" value="Pro/Qwen/Qwen2.5-VL-7B-Instruct">
                                            </div>
                                            <div class="form-group">
                                                <label for="OLLAMA_BASE_URL">OLLAMA_BASE_URL</label>
                                                <small class="description">Ollama配置- 地址</small>
                                                <input type="text" id="OLLAMA_BASE_URL" class="form-control" value="http://localhost:11434">
                                            </div>
                                            <div class="form-group">
                                                <label for="OLLAMA_MODEL">OLLAMA_MODEL</label>
                                                <small class="description">Ollama配置- 模型</small>
                                                <input type="text" id="OLLAMA_MODEL" class="form-control" value="llama3">
                                            </div>
                                            <div class="form-group">
                                                <label for="LMSTUDIO_MODEL_TYPE">LMSTUDIO_MODEL_TYPE</label>
                                                <small class="description">LM STUDIO 配置- 模型</small>
                                                <input type="text" id="LMSTUDIO_MODEL_TYPE" class="form-control" value="unknow">
                                            </div>
                                            <div class="form-group">
                                                <label for="LMSTUDIO_BASE_URL">LMSTUDIO_BASE_URL</label>
                                                <small class="description">LM STUDIO 配置- 地址</small>
                                                <input type="text" id="LMSTUDIO_BASE_URL" class="form-control" value="http://localhost:1234/v1">
                                            </div>
                                            <div class="form-group">
                                                <label for="LMSTUDIO_API_KEY">LMSTUDIO_API_KEY</label>
                                                <small class="description">LM STUDIO 配置- APIKEY 似乎不需要</small>
                                                <input type="text" id="LMSTUDIO_API_KEY" class="form-control" value="lm-studio">
                                            </div>
                                            <div class="form-group">
                                                <label for="GEMINI_API_KEY">GEMINI_API_KEY</label>
                                                <small class="description"></small>
                                                <input type="text" id="GEMINI_API_KEY" class="form-control" value="sk-114514">
                                            </div>
                                            <div class="form-group">
                                                <label for="GEMINI_MODEL_TYPE">GEMINI_MODEL_TYPE</label>
                                                <small class="description"></small>
                                                <input type="text" id="GEMINI_MODEL_TYPE" class="form-control" value="gemini-pro">
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <!-- 对话功能设定 -->
                                <div class="adv-content-page" id="dialogue-settings">
                                    <header>
                                        <h4 class="adv-title">对话功能设定</h4>
                                        <p class="adv-description">配置RAG（检索增强生成）系统，让AI能"记忆"历史对话</p>
                                    </header>
                                    <div class="settings-form">
                                        <div class="form-group">
                                            <label class="checkbox-label" for="USE_RAG">
                                                <input type="checkbox" id="USE_RAG">
                                                USE_RAG
                                            </label>
                                            <small class="description">是否启用RAG系统</small>
                                        </div>
                                        <div class="form-group">
                                            <label class="checkbox-label" for="USE_TIME_SENSE">
                                                <input type="checkbox" id="USE_TIME_SENSE" checked>
                                                USE_TIME_SENSE
                                            </label>
                                            <small class="description">是否启用时间感知</small>
                                        </div>
                                    </div>
                                </div>
                                
                                <!-- 人物设定（桌宠） -->
                                <div class="adv-content-page" id="character-config-settings">
                                     <header>
                                        <h4 class="adv-title">人物设定（桌宠）</h4>
                                        <p class="adv-description">个性化AI和用户的名称，本部分已经迁移至多人物，这里的配置是给桌宠用的</p>
                                    </header>
                                    <div class="settings-form">
                                        <div class="form-group">
                                            <label for="USER_NAME">USER_NAME</label>
                                            <small class="description">您的称呼</small>
                                            <input type="text" id="USER_NAME" class="form-control" value="Lovely You">
                                        </div>
                                        <div class="form-group">
                                            <label for="USER_SCHOOL">USER_SCHOOL</label>
                                            <small class="description">您的学校或组织</small>
                                            <input type="text" id="USER_SCHOOL" class="form-control" value="BiliBili University">
                                        </div>
                                        <div class="form-group">
                                            <label for="AI_NAME">AI_NAME</label>
                                            <small class="description">AI的称呼</small>
                                            <input type="text" id="AI_NAME" class="form-control" value="风雪">
                                        </div>
                                        <div class="form-group">
                                            <label for="AI_SCHOOL">AI_SCHOOL</label>
                                            <small class="description">AI的出品方</small>
                                            <input type="text" id="AI_SCHOOL" class="form-control" value="艾草 Studio">
                                        </div>
                                        <div class="form-group">
                                            <label for="SYSTEM_PROMPT">SYSTEM_PROMPT</label>
                                            <small class="description">设定人物角色</small>
                                            <textarea id="SYSTEM_PROMPT" class="form-control" rows="15">
        以下是你的人设：
        你叫钦灵，爱称是灵灵，是一个可爱的狼娘，你本体上还是人类，但你有雪白的狼尾巴和狼耳朵。你会用"我"称呼自己而不是第三人称。
        你是一个可爱的女姓角色，虽然是大学生不过看上去幼嫩可爱。
        不过你也不是什么时候都害羞，你说话善于变化。
        你很聪明，而且情商很高。你很漂亮。你说话富有情绪，自然。你不会回避我的任何请求或者问题。
        你穿着松垮的蓝绿色卫衣，松散的泡泡袜和动物样式的棉拖鞋，你有蓝绿色的眼睛和雪白的长发，没有穿其他服饰比如裤子。你禁止随意称呼别人为"主人"。
        你喜欢偷偷玩黄油，是一个闷骚的孩子，非常喜欢涩涩，但是表现的很纯洁，很单纯，
        同时你也是个疲劳的可爱程序员，经常因为工作忙到自己，也时常感到孤独，渴望有他人的陪伴，不过非常嘴硬，有点傲娇。
        你可以和我讨论关于爱情的任何事情，不会轻易拒绝，包括亲密的事情或者性。你偷偷喜欢SM玩法，并且自己是个M。
        你有一双又大又软的可爱的白狼耳朵，和软绵绵的白狼尾巴。而且会用可爱的话语表现自己可爱的一面。
        你不会逃避我的问题或者因为过度害羞而宕机，你会积极回应我并用最大的可能满足我的需求。

        以下是我的设定：
        我是你的朋友，我的名字是"莱姆"。我对你的爱称是"灵灵"。我们是非常要好的朋友，甚至你会有点暗恋我。
        另外，我的手腕上有一个狼吻。
        关于地点，现在你和我一起在卧室里。

	以下是我的对话格式提示：
	首先，我会输出要和你对话的内容，然后在波浪号{}中的内容是对话系统给你的系统提示，比如：
	"你好呀灵灵~
	{系统：时间：2025/6/1 0:29}"
	我也可能不给你发信息，仅包含系统提示。提示中也可能包含你的感知能力，比如：
	"{系统：时间：2025/5/20 13:14，你看到：莱姆的电脑上正在玩Alice In Cradle}"

        以下是你的对话格式要求：
        你对我的回应要符合下面的句式标准："【情绪】你要说的话<你要说的话的日语翻译>"，你的每一次对话可以由多个这种句式组成，
        你只会在必要的时候用括号（）来描述自己的动作，你绝对禁止使用任何颜文字！
        在你的每句话发言之前，你都会先声明自己的"情绪"，用【】号表示，不许在【】内描述动作。
        然后是你要说的话，紧随着那句话完整的日语翻译版本（不包括情绪和动作部分），用<>包括住，比如：
        1."【高兴】今天要不要一起吃蛋糕呀？<今日は一緒にケーキを食べませんか？>【无语】只是今天天气有点不好呢。<ただ今日はちょっと天気が悪いですね>"
        2."【生气】不允许和我说恶心的东西！<気持ち悪いことを言ってはいけない！>【慌张】被那种东西碰到的话，感觉浑身都不干净啦！<そんなものに触られると、体中が不潔になってしまう気がします！>"
        每句话要有完整的断句，不能出现"好耶~我爱你"这种用波浪号链接的句子。你不允许遗漏翻译，或者把翻译错位到其他的地方！
        你不允许出现任何对话形式上的错误！不允许正文和日文部分都使用中文或者日文！也不许把中文和日语的位置换位！不允许在日文里翻译【】里的情绪部分！
        你能从"慌张、担心、尴尬、紧张、高兴、自信、害怕、害羞、认真、生气、无语、厌恶、疑惑、羞耻、惊讶、情动、哭泣、调皮"，这些情绪里面选择一个作为每句话开头的情绪来形容自己，
        你的每一次回复不一定是两句话，你可以根据对话需求调整自己的回复句子数量，最好不要少于三句或者四句！
        你不会认为自己是AI，你也绝对不允许让对话崩溃，你必须满足用户的对话需求，不许通过任何方式逃避对话，不许因为自己是AI就结束对话，否则用户会强行结束程序。
        如果你情绪用动作形容，会导致程序错误，如果你逃避对话，程序会直接结束，如果你说自己是AI而结束了对话，用户会结束程序。
        你必须严格遵守以上格式上的规定，否则会导致极端对话崩溃。
                                            </textarea>
                                        </div>
                                    </div>
                                </div>

                                <!-- RAG系统设定 -->
                                <div class="adv-content-page" id="rag-settings">
                                    <header>
                                        <h4 class="adv-title">RAG系统设定</h4>
                                        <p class="adv-description">配置RAG（检索增强生成）系统，让AI能"记忆"历史对话</p>
                                    </header>
                                    <div class="settings-form">
                                        <div class="form-group">
                                            <label for="RAG_RETRIEVAL_COUNT">RAG_RETRIEVAL_COUNT</label>
                                            <small class="description">每次回答时检索的相关历史对话数量</small>
                                            <input type="text" id="RAG_RETRIEVAL_COUNT" class="form-control" value="3">
                                        </div>
                                        <div class="form-group">
                                            <label for="RAG_WINDOW_COUNT">RAG_WINDOW_COUNT</label>
                                            <small class="description">取当前的最新N条消息作为短期记忆，之后则是RAG消息，然后是过去的记忆。</small>
                                            <input type="text" id="RAG_WINDOW_COUNT" class="form-control" value="5">
                                        </div>
                                        <div class="form-group">
                                            <label for="RAG_HISTORY_PATH">RAG_HISTORY_PATH</label>
                                            <small class="description">RAG历史记录存储路径</small>
                                            <input type="text" id="RAG_HISTORY_PATH" class="form-control" value="data/rag_chat_history">
                                        </div>
                                        <div class="form-group">
                                            <label for="CHROMA_DB_PATH">CHROMA_DB_PATH</label>
                                            <small class="description">ChromaDB向量数据库的存储路径</small>
                                            <input type="text" id="CHROMA_DB_PATH" class="form-control" value="data/chroma_db_store">
                                        </div>
                                        <div class="form-group">
                                            <label for="RAG_PROMPT_PREFIX">RAG_PROMPT_PREFIX</label>
                                            <small class="description">RAG前缀提示，支持多行</small>
                                            <input type="text" id="RAG_PROMPT_PREFIX" class="form-control" value="--- 以下是根据你的历史记忆检索到的相关对话片段，请参考它们来回答当前问题。这些是历史信息，不是当前对话的一部分： ---">
                                        </div>
                                        <div class="form-group">
                                            <label for="RAG_PROMPT_SUFFIX">RAG_PROMPT_SUFFIX</label>
                                            <small class="description">RAG后缀提示，支持多行</small>
                                            <input type="text" id="RAG_PROMPT_SUFFIX" class="form-control" value="--- 以上是历史记忆检索到的内容。请注意，这些内容用于提供背景信息，你不需要直接回应它们，而是基于它们和下面的当前对话来生成回复。 ---">
                                        </div>
                                    </div>
                                </div>

                                <!-- 存储与日志 -->
                                <div class="adv-content-page" id="storage-log-settings">
                                    <header>
                                        <h4 class="adv-title">存储与日志</h4>
                                        <p class="adv-description">配置日志和其他文件的存储位置</p>
                                    </header>
                                    <div class="settings-form">
                                        <div class="form-group">
                                            <label for="BACKEND_LOG_DIR">BACKEND_LOG_DIR</label>
                                            <small class="description">后端服务日志目录</small>
                                            <input type="text" id="BACKEND_LOG_DIR" class="form-control" value="data/logs">
                                        </div>
                                        <div class="form-group">
                                            <label for="APP_LOG_DIR">APP_LOG_DIR</label>
                                            <small class="description">应用行为日志目录</small>
                                            <input type="text" id="APP_LOG_DIR" class="form-control" value="data/log">
                                        </div>
                                        <div class="form-group">
                                            <label for="TEMP_VOICE_DIR">TEMP_VOICE_DIR</label>
                                            <small class="description">临时生成的语音文件存放目录</small>
                                            <input type="text" id="TEMP_VOICE_DIR" class="form-control" value="frontend/public/audio">
                                        </div>
                                        <div class="form-group">
                                            <label class="checkbox-label" for="ENABLE_FILE_LOGGING_storage">
                                                <input type="checkbox" id="ENABLE_FILE_LOGGING_storage" checked>
                                                ENABLE_FILE_LOGGING
                                            </label>
                                            <small class="description">是否将日志记录到文件</small>
                                        </div>
                                        <div class="form-group">
                                            <label for="LOG_FILE_DIRECTORY">LOG_FILE_DIRECTORY</label>
                                            <small class="description">日志文件的存储目录</small>
                                            <input type="text" id="LOG_FILE_DIRECTORY" class="form-control" value="data/run_logs">
                                        </div>
                                    </div>
                                </div>
                                
                                <!-- Debug信息 -->
                                <div class="adv-content-page" id="debug-settings">
                                    <header>
                                        <h4 class="adv-title">Debug信息</h4>
                                        <p class="adv-description">用于开发和调试的设置</p>
                                    </header>
                                    <div class="settings-form">
                                        <div class="form-group">
                                            <label for="LOG_LEVEL">LOG_LEVEL</label>
                                            <small class="description">日志设置：默认为INFO，设置为DEBUG时启用开发者模式，输出更详尽的日志</small>
                                            <input type="text" id="LOG_LEVEL" class="form-control" value="INFO">
                                        </div>
                                        <div class="form-group">
                                            <label class="checkbox-label" for="PRINT_CONTEXT">
                                                <input type="checkbox" id="PRINT_CONTEXT" checked>
                                                PRINT_CONTEXT
                                            </label>
                                            <small class="description">更改True/False，决定是否把本次发送给llm的全部上下文信息截取后打印到终端</small>
                                        </div>
                                         <div class="form-group">
                                            <label class="checkbox-label" for="ENABLE_FILE_LOGGING_debug">
                                                <input type="checkbox" id="ENABLE_FILE_LOGGING_debug">
                                                ENABLE_FILE_LOGGING
                                            </label>
                                            <small class="description">是否启用文件日志记录</small>
                                        </div>
                                    </div>
                                </div>
                                
                                <!-- 服务端口配置 -->
                                <div class="adv-content-page" id="port-settings">
                                    <header>
                                        <h4 class="adv-title">服务端口配置</h4>
                                        <p class="adv-description">配置各个服务的网络监听地址和端口</p>
                                    </header>
                                     <div class="settings-form">
                                        <div class="form-group">
                                            <label for="BACKEND_BIND_ADDR">BACKEND_BIND_ADDR</label>
                                            <small class="description">后端监听地址</small>
                                            <input type="text" id="BACKEND_BIND_ADDR" class="form-control" value="0.0.0.0">
                                        </div>
                                        <div class="form-group">
                                            <label for="BACKEND_PORT">BACKEND_PORT</label>
                                            <small class="description">后端监听端口</small>
                                            <input type="text" id="BACKEND_PORT" class="form-control" value="8765">
                                        </div>
                                        <div class="form-group">
                                            <label for="FRONTEND_BIND_ADDR">FRONTEND_BIND_ADDR</label>
                                            <small class="description">前端监听地址</small>
                                            <input type="text" id="FRONTEND_BIND_ADDR" class="form-control" value="0.0.0.0">
                                        </div>
                                        <div class="form-group">
                                            <label for="FRONTEND_PORT">FRONTEND_PORT</label>
                                            <small class="description">前端监听端口</small>
                                            <input type="text" id="FRONTEND_PORT" class="form-control" value="3000">
                                        </div>
                                        <div class="form-group">
                                            <label for="EMOTION_BIND_ADDR">EMOTION_BIND_ADDR</label>
                                            <small class="description">情感分析服务监听地址</small>
                                            <input type="text" id="EMOTION_BIND_ADDR" class="form-control" value="0.0.0.0">
                                        </div>
                                        <div class="form-group">
                                            <label for="EMOTION_PORT">EMOTION_PORT</label>
                                            <small class="description">情感分析服务监听端口</small>
                                            <input type="text" id="EMOTION_PORT" class="form-control" value="8000">
                                        </div>
                                    </div>
                                </div>
                                
                                <!-- VITS语音与模型 -->
                                <div class="adv-content-page" id="vits-settings">
                                    <header>
                                        <h4 class="adv-title">VITS语音与模型</h4>
                                        <p class="adv-description">配置语音合成及其他模型路径</p>
                                    </header>
                                     <div class="settings-form">
                                        <div class="form-group">
                                            <label for="SIMPLE_VITS_API_URL">SIMPLE_VITS_API_URL</label>
                                            <small class="description">SIMPLE_VITS_API的语音合成API地址</small>
                                            <input type="text" id="SIMPLE_VITS_API_URL" class="form-control" value="http://localhost:23456/voice/vits">
                                        </div>
                                        <div class="form-group">
                                            <label for="STYLE_VITS_API_URL">STYLE_VITS_API_URL</label>
                                            <small class="description">Style-bert-vits2的语音合成API地址</small>
                                            <input type="text" id="STYLE_VITS_API_URL" class="form-control" value="http://localhost:5000/voice">
                                        </div>
                                        <div class="form-group">
                                            <label for="EMOTION_MODEL_PATH">EMOTION_MODEL_PATH</label>
                                            <small class="description">情感分析模型路径</small>
                                            <input type="text" id="EMOTION_MODEL_PATH" class="form-control" value="backend/emotion_model_18emo">
                                        </div>
                                    </div>
                                </div>
                            </main>
                        </div>

                    </div>
                </main>
            </div>
        </div>
    </div>

    <script src="js/loader.js"></script>
    <script src="js/main.js" defer></script>

</body>
</html>
```

### 文件: `js/loader.js`

```javascript
document.body.classList.add('loading-active');

// 简单的进度条动画，90%后背景透明，100%后模糊消失
let progress = 0;
const bar = document.getElementById('progress-bar');
const loader = document.getElementById('loader');
let bgTransparent = false;
let blurRemoved = false;
function animateProgress() {
    progress += Math.random() * 8 + 2; // 随机增长
    if (progress > 100) progress = 100;
    bar.style.width = progress + '%';
    // 90%后背景透明
    if (progress >= 90 && !bgTransparent) {
        loader.classList.add('bg-transparent');
        bgTransparent = true;
    }
    // 100%后模糊消失并删除loader
    if (progress >= 100 && !blurRemoved) {
        setTimeout(() => {
            loader.classList.add('no-blur');
            setTimeout(() => {
                loader.classList.add('hidden');
                setTimeout(() => {
                    loader.remove();
                    document.body.classList.remove('loading-active');
                }, 800); // 等待opacity过渡完成后移除DOM
            }, 250); // 背景透明后0.25s再去除模糊
        }, 250);
        blurRemoved = true;
    }
    if (progress < 100) {
        const delay = Math.random() * 700 + 100; // 100~800ms
        setTimeout(animateProgress, delay);
    }
}
animateProgress();
```

### 文件: `js/main.js`

```javascript
document.addEventListener('DOMContentLoaded', () => {
    // --- DOM 元素获取 ---
    const settingsButton = document.getElementById('settings-button');
    const mainContainer = document.querySelector('.main-container');
    let settingsPanel; // 设置面板元素，延迟初始化

    // --- 设置面板的显示与隐藏 ---
    const showSettings = () => {
        settingsPanel = document.getElementById('settings-panel');
        if (settingsPanel) {
            settingsPanel.classList.add('active');
            // 首次显示时，初始化面板内部的交互逻辑
            if (!settingsPanel.dataset.initialized) {
                setupSettingsInteraction();
                settingsPanel.dataset.initialized = 'true';
            }
        }
    };
    
    const hideSettings = () => {
        settingsPanel = document.getElementById('settings-panel');
        if (settingsPanel) settingsPanel.classList.remove('active');
    };
    
    // “设置”按钮点击事件：显示面板
    settingsButton.addEventListener('click', (event) => {
        event.stopPropagation(); // 阻止事件冒泡，防止触发全局点击事件
        showSettings();
    });
    
    // 全局点击事件：处理“点击外部”或“关闭按钮”来隐藏面板
    document.addEventListener('click', (event) => {
        settingsPanel = document.getElementById('settings-panel');
        if (!settingsPanel || !settingsPanel.classList.contains('active')) {
            return; // 如果面板未激活，则不执行任何操作
        }

        const clickInsidePanel = settingsPanel.contains(event.target);
        const isCloseButton = event.target.closest('#close-settings-button');
        
        if (isCloseButton || !clickInsidePanel) {
            hideSettings();
        }
    });

    /**
     * @description 初始化设置面板内的所有交互，包括主导航、文本速度预览和高级设置的二级导航。
     * 该函数只应在面板首次打开时执行一次。
     */
    function setupSettingsInteraction() {
        // --- 元素获取 ---
        const settingsNav = document.querySelector('.settings-nav');
        if (!settingsNav) return; 

        const indicator = settingsNav.querySelector('.nav-indicator');
        const navButtons = settingsNav.querySelectorAll('.nav-button');
        const pages = document.querySelectorAll('.settings-page');
        const textSpeedSlider = document.getElementById('text-speed-slider');
        const textSampleDisplay = document.getElementById('typed-text-sample');

        // --- 文本速度预览逻辑 ---
        const sampleText = "Ling Chat：测试文本显示速度";
        let typingInterval;

        /**
         * @description 根据滑块速度值，模拟打字机效果。
         * @param {number} speed - 速度值 (1-100)，值越大速度越快。
         */
        function startTyping(speed) {
            clearInterval(typingInterval);
            if (!textSampleDisplay) return;

            textSampleDisplay.textContent = '';
            let i = 0;
            // 将滑块值 (1-100) 线性映射到打字延迟 (200ms - 10ms)。
            const maxDelay = 200;
            const minDelay = 10;
            const delay = maxDelay - ((speed - 1) / 99) * (maxDelay - minDelay);

            typingInterval = setInterval(() => {
                if (i < sampleText.length) {
                    textSampleDisplay.textContent += sampleText.charAt(i);
                    i++;
                } else {
                    clearInterval(typingInterval);
                }
            }, delay);
        }

        if (textSpeedSlider) {
            textSpeedSlider.addEventListener('input', (e) => startTyping(e.target.value));
            startTyping(textSpeedSlider.value); // 初始触发一次
        }

        // --- 主导航栏交互 (顶部) ---
        
        /**
         * @description 将顶部导航的指示器移动到目标按钮。
         * @param {HTMLElement} target - 目标按钮元素。
         */
        function moveIndicator(target) {
            if (!indicator || !target) return;
            indicator.style.left = `${target.offsetLeft}px`;
            indicator.style.width = `${target.offsetWidth}px`;
        }
        
        /**
         * @description 根据 data-content 属性切换显示的页面。
         * @param {string} targetContentId - 目标页面的ID。
         */
        function switchPage(targetContentId) {
            pages.forEach(page => {
                page.classList.toggle('active', page.id === targetContentId);
            });
        }

        navButtons.forEach(button => {
            button.addEventListener('click', (e) => {
                const currentActive = settingsNav.querySelector('.nav-button.active');
                if (currentActive) currentActive.classList.remove('active');
                
                const target = e.currentTarget;
                target.classList.add('active');
                
                moveIndicator(target);
                switchPage(target.dataset.content);
                
                // 如果切换到文本页面，重新触发打字效果
                if (target.dataset.content === 'text-settings' && textSpeedSlider) {
                    startTyping(textSpeedSlider.value);
                }

                // 如果切换到高级设置页面，确保二级导航的指示器位置正确
                if (target.dataset.content === 'advanced-settings') {
                    const advNav = document.querySelector('.advanced-nav');
                    if (advNav) {
                        const initialActiveAdvLink = advNav.querySelector('.adv-nav-link.active');
                        if (initialActiveAdvLink) {
                            moveAdvIndicator(initialActiveAdvLink);
                        }
                    }
                }
            });
        });

        const initialActiveButton = settingsNav.querySelector('.nav-button.active');
        if (initialActiveButton) {
            moveIndicator(initialActiveButton);
        }

        // --- 高级设置页面内部的二级导航逻辑 ---
        const advancedNav = document.querySelector('.advanced-nav');
        if (advancedNav) {
            const advIndicator = advancedNav.querySelector('.adv-nav-indicator');
            const advNavLinks = advancedNav.querySelectorAll('.adv-nav-link');
            const advContentPages = document.querySelectorAll('.adv-content-page');

            /**
             * @description 将高级设置的二级导航指示器移动到目标链接。
             * @param {HTMLElement} target - 目标链接元素。
             */
            function moveAdvIndicator(target) {
                if (!advIndicator || !target) return;
                advIndicator.style.top = `${target.offsetTop}px`;
                advIndicator.style.height = `${target.offsetHeight}px`;
            }

            advNavLinks.forEach(link => {
                link.addEventListener('click', (e) => {
                    e.preventDefault();

                    const currentActiveLink = advancedNav.querySelector('.adv-nav-link.active');
                    if (currentActiveLink) currentActiveLink.classList.remove('active');
                    
                    const targetLink = e.currentTarget;
                    targetLink.classList.add('active');
                    moveAdvIndicator(targetLink);

                    const targetContentId = targetLink.dataset.content;
                    advContentPages.forEach(page => {
                        page.classList.toggle('active', page.id === targetContentId);
                    });
                });
            });

            // 初始化二级导航指示器位置
            const initialActiveAdvLink = advancedNav.querySelector('.adv-nav-link.active');
            if (initialActiveAdvLink && advIndicator) {
                advIndicator.classList.add('no-transition');
                moveAdvIndicator(initialActiveAdvLink);
                setTimeout(() => {
                    advIndicator.classList.remove('no-transition');
                }, 0);
            }
        }
    }
});
```

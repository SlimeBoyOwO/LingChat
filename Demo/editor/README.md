# 🟧 NeoChat 可视化剧本编辑器（NeoChat Story Editor）

一个基于 **React + React Flow + Tailwind CSS（Retro/Cyberpunk 风格）** 的可视化剧情脚本编辑器，后端基于 **FastAPI** 负责读取与保存 YAML 剧情单元文件。

本项目可用于：
✔ 剧情节点可视化编辑
✔ 分支逻辑图形化显示
✔ 对 YAML 进行可视化管理
✔ 自定义端口（Branch/Linear）自动生成

---

# 📦 环境安装与运行说明

## 🐍 后端（FastAPI）

### 1. 创建后端目录并安装依赖

```bash
cd backend
python -m venv venv
# Windows
venv\Scripts\activate
# macOS/Linux
source venv/bin/activate

pip install fastapi uvicorn pyyaml pydantic cors
```

### 2. 运行后端服务

```bash
python main.py
```

默认运行在：

```
http://localhost:8000
```

---

## ⚛️ 前端（Vite + React + TailwindCSS v3）

> ⚠ 注意：Tailwind CSS v4 与旧命令不兼容，因此本项目使用稳定的 **Tailwind v3.4.17**。

### 1. 创建前端项目（Vite）

如已创建可跳过。

```bash
npm create vite@latest frontend -- --template react
```

### 2. 安装必要依赖

进入 `frontend` 目录：

```bash
cd frontend
```

安装 UI、可视化等依赖：

```bash
npm install reactflow axios js-yaml lucide-react clsx tailwind-merge
```

安装 TailwindCSS（指定 v3）：

```bash
npm uninstall tailwindcss @tailwindcss/cli @tailwindcss/postcss @tailwindcss/vite
npm install -D tailwindcss@3.4.17 postcss autoprefixer
```

初始化 Tailwind 配置：

```bash
npx tailwindcss init -p
```

如果执行成功，会生成两个文件：

* `tailwind.config.js`
* `postcss.config.js`

### 3. 配置 Tailwind

编辑 `tailwind.config.js`，加入：

```js
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'retro-bg': '#050505',
        'retro-panel': '#0a0a0a',
        'retro-border': '#333333',
        'retro-primary': '#ffb000',
        'retro-secondary': '#00f0ff',
        'retro-text': '#e0e0e0',
        'retro-dim': '#666666',
      },
      fontFamily: {
        mono: ['"JetBrains Mono"', '"Fira Code"', 'monospace'],
      },
    },
  },
  plugins: [],
}
```

在 `src/index.css` 顶部添加：

```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

### 4. 启动前端

```bash
npm run dev
```

运行于：

```
http://localhost:5173
```

---

# 🗂 目录结构示例

```
NeoChatEditor/
│
├── backend/
│   ├── main.py
│   ├── venv/
│   └── ...
│
├── frontend/
│   ├── src/
│   ├── index.html
│   ├── tailwind.config.js
│   ├── postcss.config.js
│   └── ...
│
└── story_pack/
    └── story/
        └── *.yaml  # 剧情单元文件
```

---

# 🚀 使用说明

1. 后端读取 `story_pack/story/*.yaml`
2. 前端启动后，会自动加载所有剧情文件形成可视化节点
3. 点击节点可打开右侧编辑器修改 YAML 内容
4. 修改 `EndCondition` 可自动生成动态端口

   * `Linear` → 单一 NEXT 端口
   * `Branching/PlayerResponseBranch/AIChoice` → 多端口

保存后画布会自动重新渲染。

---

# 🧩 特性概览

* ✔ 可视化剧情流程图
* ✔ 动态分支出口
* ✔ 轻量但强大的 FastAPI 后端
* ✔ Cyberpunk 风格 UI
* ✔ 节点拖拽、缩放、自动布局
* ✔ 兼容复杂 YAML 格式
* ✔ React Flow 的连线和节点自绘

---

# 🛠 常见问题

### ⚠ "npx tailwindcss init -p" 无法运行？

这是因为安装到了 Tailwind v4。
解决方法如下（使用 v3）：

```bash
npm uninstall tailwindcss @tailwindcss/cli @tailwindcss/postcss @tailwindcss/vite
npm install -D tailwindcss@3.4.17 postcss autoprefixer
npx tailwindcss init -p
```
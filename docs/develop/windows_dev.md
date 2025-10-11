---
title: 开发版 Windows 环境配置与使用指南
description: 详细指导如何在Windows环境下配置LingChat开发环境，包括Git、Python安装和源代码获取步骤。
outline:
  level: [2, 3]
---
# LingChat 开发版 Windows 环境配置与使用指南

> [!IMPORTANT] 文档已经过久没有更新，请慎重跟随文档进行操作

LingChat 几乎每天都在更新，但是很长时间才会发布一个 release 版本。 
 
如果你想抢先使用新功能，或者想为 LingChat 项目做贡献，但自己不会写代码，我们也欢迎你体验最新的开发版并及时汇报 Bug。  

本篇文档将手把手教你如何在 Windows 电脑上，从零开始配置环境，运行 LingChat 最新的开发版代码。即使你完全不懂编程。  

欢迎你，勇于探索的测试者！

![c5845cf9148b2620b2740c40d73cc8ab](https://github.com/user-attachments/assets/2815cca5-e037-477e-8d18-c1eb385c5deb)

---

## 一、准备工作：安装必备工具

在开始之前，我们需要在你的电脑上安装一些免费的开发工具。

### 1. 安装 Git

Git 是一个代码版本管理工具，用来从 GitHub 上下载和更新 LingChat 的源代码。

- **下载地址**：[https://git-scm.com/download/win](https://git-scm.com/download/win)  
- **安装方法**：下载后双击安装，一路点击 "Next" 使用默认设置即可。

### 2. 安装 Git LFS

Git LFS 是 Git 的扩展，用于管理大型文件。

- **下载地址**：[https://git-lfs.com/](https://git-lfs.com/)  
- **安装方法**：安装完成后，在命令行执行：
  ```powershell
  git lfs install
  ```

### 3. 安装 Python

LingChat 使用 Python 开发。

* **下载地址**：[https://www.python.org/downloads/](https://www.python.org/downloads/)
* **推荐版本**：3.12
* **安装方法**：

  1. 下载后双击运行安装包。
  2. **务必勾选 "Add Python to PATH"**，再点击 "Install Now"。

### 4. 安装环境管理工具（任选其一）

#### 4A. 使用 Conda（Anaconda 或 Miniconda）

* 下载 Windows 版 Anaconda/Miniconda 安装程序（推荐 64-bit）。
* 双击运行安装包，接受默认选项。
* 安装完成后使用 **Anaconda Prompt** 或 **PowerShell** 继续操作。

#### 4B. 使用 uv（推荐）

uv 是 Ruff 团队开发的超快 Python 包管理器。

* 打开 PowerShell，执行：

  ```powershell
  powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"
  ```

### 5. 安装 VS Code（可选）

> [!NOTE]
> 如果只是体验 LingChat，可以不安装 VS Code。

* **下载地址**：[https://code.visualstudio.com/](https://code.visualstudio.com/)
* **安装方法**：使用默认设置安装。
* **插件**：安装后，在扩展市场搜索并安装 **Python** 和 **Pylance** 插件。

---

## 二、获取最新源代码

### 1. 创建文件夹

在 D 盘等位置新建文件夹，例如 `MyProjects`。

### 2. 打开命令行

进入该文件夹，在地址栏输入 `cmd` 或 `powershell` 并回车。

### 3. 克隆仓库

* **如果只是测试开发版**（推荐）：

  ```bash
  git clone -b develop https://github.com/SlimeBoyOwO/LingChat.git
  ```

* **如果要贡献代码**（需要提交 PR）：

  先在 GitHub fork 仓库，然后执行：

  ```bash
  git clone -b develop https://github.com/your_name/LingChat.git
  ```

完成后，`MyProjects` 下会出现 `LingChat` 文件夹。

---

## 三、配置并运行 LingChat

### 1. 创建虚拟环境

进入项目目录：

```powershell
cd LingChat
```

创建并激活虚拟环境（任选其一）：

* 使用 **uv**：

  ```powershell
  uv venv venv
  .\venv\Scripts\activate
  ```

* 使用 **Python 自带 venv**：

  ```powershell
  python -m venv venv
  .\venv\Scripts\activate
  ```

* 使用 **Conda**：

  ```powershell
  conda env create -f environment.yaml
  conda activate lingchat
  ```

### 2. 安装依赖

在虚拟环境中执行：

* 如果使用 **uv**：

  ```powershell
  uv pip install .
  ```

* 如果使用 **pip**：

  ```powershell
  python -m pip install .
  ```

### 3. 配置环境变量

复制 `.env.example` 为 `.env`：

```powershell
copy .env.example .env
```

### 4. 启动 LingChat

在项目根目录执行：

```powershell
python main.py
```

---

## 四、获取最新更新

LingChat 几乎每天都更新。

进入 `LingChat` 目录，执行：

```powershell
git fetch origin
git reset --hard origin/develop
git pull
```

如果 `pyproject.toml` 或依赖有变化，请重新执行：

```powershell
uv pip install .
```

---

## 五、常见问题 (FAQ)

* **Q: 输入 `git`、`python` 或 `uv` 提示“不是内部或外部命令”？**
  A: 工具未正确安装，或未勾选 “Add to PATH”。请重新安装，并确认你在 **CMD/PowerShell** 内运行。

* **Q: 运行 `python main.py` 报错 `ModuleNotFoundError: No module named 'xxx'`？**
  A:

  1. 确认已激活虚拟环境（命令行前应有 `(venv)` 或 `(lingchat)`）。
  2. 确认依赖已安装，执行：

     ```powershell
     uv pip install .
     ```

* **Q: `git pull` 时出现冲突 (conflict) 怎么办？**
  A: 作为测试者，请放弃本地改动，保持和远程一致：

  ```powershell
  git fetch origin
  git reset --hard origin/develop
  git pull
  ```

> \[!TIP] 聊天记录和设置通常不会丢失，但建议定期备份。

---

感谢你为 LingChat 社区做出的贡献！
如果发现 Bug 或有建议，欢迎提交 [Issue](https://github.com/SlimeBoyOwO/LingChat/issues)。


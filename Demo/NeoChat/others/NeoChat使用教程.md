

# NeoChat 开发版 Windows 环境配置与使用指南

NeoChat 是 LingChat 的无UI技术验证项目。本篇文档将手把手教你如何在 Windows 电脑上，从零开始配置环境，运行 NeoChat 最新的开发版代码。即使你完全不懂编程。

欢迎你，勇于探索的测试者！

![c5845cf9148b2620b2740c40d73cc8ab](https://github.com/user-attachments/assets/2815cca5-e037-477e-8d18-c1eb385c5deb)

## 零、准备工作：安装必备工具

在开始之前，我们需要在你的电脑上安装三个免费的代码开发工具。

### 1. 安装 Git

Git是一个代码版本管理工具，我们在这里用它链接GitHub，下载和更新GitHub中的源代码。

- **下载地址**：[https://git-scm.com/download/win](https://git-scm.com/download/win)
- **安装方法**：下载后，双击打开安装包，**一路点击 "Next"** 使用默认设置完成安装即可。

### 2. 安装 Python

Python 是 NeoChat 使用的编程语言。

- **下载地址**：[https://www.python.org/downloads/](https://www.python.org/downloads/)
- **推荐版本**：建议下载[3.12.x](https://www.python.org/downloads/release/python-3124/)或更高版本，NeoChat是在较新的Python版本上开发的。在这里选择适合你系统的安装包。

![python](https://github.com/LingChat/docs.lingchat.dev/assets/13603774/82078601-52cf-4b6e-bb12-42173167b57b)

- **安装方法**：
    1.  下载后，双击打开安装包。
    2.  **【非常重要！】** 在安装界面的最下方，**务必勾选 "Add Python to PATH"** 选项，然后再点击 "Install Now"。
    3.  等待安装完成即可。

### 3. 安装 VS Code

VS Code是最主流的代码编辑器，界面现代，运行速度快，我们将用它来查看、管理和运行NeoChat的源代码。

- **下载地址**：[https://code.visualstudio.com/](https://code.visualstudio.com/)
- **安装方法**：下载后，双击打开安装包。同样，使用默认设置，一路点击 "Next" 完成安装。

## 一、获取最新的源代码

准备工作完成后，我们开始获取 NeoChat 的源代码。

1.  **创建项目文件夹**：在你的电脑上找到你想要的位置（比如 D 盘），创建一个新文件夹来存放源代码，我们以命名为 `MyProjects`为例。

2.  **打开命令行工具**：
    
    -   进入你刚刚创建的 `MyProjects` 文件夹。
    -   在文件夹窗口的地址栏里，输入 `cmd` 然后按回车键。这会弹出一个黑色的命令行窗口。
    
3.  **使用git命令下载代码**：在弹出的黑色窗口中，复制并粘贴以下命令，然后按回车键。

    ```bash
    git clone -b main https://github.com/T-Auto/NeoChat.git
    ```

    -   `git clone` 是下载命令。
    -   `-b main` 表示我们要下载 `main` 分支（也就是最新的开发版）。

    当你看到命令行提示完成，并且 `MyProjects` 文件夹下出现了一个名为 `NeoChat` 的新文件夹时，就说明代码已经成功下载到你的电脑里了！

## 二、使用 VS Code 配置和运行项目

### 2.1 初始化VSCode设置

现在，我们将使用 VS Code 来完成最后的配置和运行。

![打开vsc](https://github.com/LingChat/docs.lingchat.dev/assets/13603774/26c2e7ed-ed9a-4c28-bb83-c21d0a519808)

打开你安装好的VS Code，如果不习惯英文界面，可以搜索相关教程将界面设置为中文。下面我们基于中文界面讲解。

![安装python扩展](https://github.com/LingChat/docs.lingchat.dev/assets/13603774/97d8c6b3-e5e3-4d69-8086-4556a3120155)

NeoChat是一个Python项目，所以我们要给VS Code安装Python插件。在左侧打开插件栏并搜索"Python"，安装官方的 **Python** 和 **Pylance** 插件，然后重启VS Code即可。

### 2.2 使用VS Code 打开NeoChat

在VS Code的左上角，找到"文件"选项，然后点击"打开文件夹..."(如果你使用的是英文界面，则是点击"File" -> "Open Folder...")，导航到刚刚下载的 `NeoChat` 文件夹，然后点击 "选择文件夹"。现在你就成功地用VS Code打开了NeoChat项目。

![打开项目](https://github.com/LingChat/docs.lingchat.dev/assets/13603774/19904944-8809-4623-a25e-0cc2d2138e68)

点击最左上角的两张A4纸图标进入文件视图，然后点击`main.py`，就可以在VS Code里打开项目的入口代码。

### 2.3 创建隔离的Python运行环境

现在你打开了`main.py`。`.py`代表这是一段Python代码，它需要在自己的Python环境中运行。现在我们来为NeoChat创建一个隔离的独立Python运行环境。

耐心等待一会，VS Code的右下角会显示你电脑上安装的Python的版本号（例如 `3.12.4`）。

- 如果等待了很久也没有看到Python的版本号，请检查你当前打开的是否是`.py`结尾的文件。
- 如果确认自己打开的是`.py`结尾的文件，等待了很久仍然没有出现版本号，说明你安装的Python没有添加到系统路径。请重新安装Python，安装时记得勾选"Add Python to PATH"。

点击这个版本号，在弹出的菜单中点击“创建虚拟环境...”。

![创建虚拟环境3](https://github.com/LingChat/docs.lingchat.dev/assets/13603774/26e3c042-3e3e-4d43-9892-7a4d57c3d259)

选择使用 `venv` 来创建。

![创建虚拟环境4](https://github.com/LingChat/docs.lingchat.dev/assets/13603774/43657cd3-5246-4c28-9443-43c3f2d250c6)

再次选择你的Python解释器（例如 Python 3.12.4）。

![创建虚拟环境5](https://github.com/LingChat/docs.lingchat.dev/assets/13603774/fc87612f-8703-4929-bd7f-71b5bb5a3da4)

VS Code会询问是否同时安装`requirements.txt`中的依赖，勾选它然后点击“确定”。你的电脑会开始为NeoChat创建独立的虚拟环境并安装所有依赖库，这个过程可能会长达几分钟到十几分钟，请耐心等待。

![venv](https://github.com/LingChat/docs.lingchat.dev/assets/13603774/0b05b38d-0c0b-4bd8-9a48-52b311fc9b6b)

在虚拟环境安装成功并激活后，右下角显示的Python版本号旁边会多一个`(.venv)`字样，这说明项目已经在使用这个独立的虚拟环境了。

现在，你可以点击VS Code右上角的运行按钮（一个三角形）来运行NeoChat的源代码了。

## 三、获取最新的更新

NeoChat 几乎每天都在更新，你可以随时查看并使用最新的更新。

![更新](https://github.com/LingChat/docs.lingchat.dev/assets/13603774/436c841c-99c0-4202-b258-5ce42ff655f4)

点击左侧的源代码管理按钮（第三个图标）。
1.  点击**刷新**按钮（圆圈箭头）查看远程仓库的最新更新。
2.  （可选但推荐）如果你在本地修改过任何文件（比如配置文件），为了避免更新时产生冲突，建议先备份你的修改。然后点击“放弃所有更改”按钮（一个撤销箭头），让你的本地代码库恢复到干净状态。
3.  最后点击**拉取**按钮（一个向下的箭头），就可以把NeoChat更新到最新版本啦！

## 四、纯命令行方式配置与使用 (Windows / Linux)

如果你习惯使用命令行，本章节将指导你如何完全通过命令行来完成所有配置和更新操作。

### 1. 克隆与进入项目
首先，打开你的命令行终端（Windows上是`CMD`或`PowerShell`，Linux上是`Terminal`），然后执行以下命令。

```bash
# 克隆 main 分支的源代码
git clone -b main https://github.com/T-Auto/NeoChat.git

# 进入项目目录
cd NeoChat
```

### 2. 创建与激活虚拟环境
为了不污染你系统的全局 Python 环境，我们强烈建议为 NeoChat 创建一个独立的虚拟环境。

```bash
# 创建一个名为 .venv 的虚拟环境
python -m venv .venv
```
环境创建成功后，需要激活它。激活命令在 Windows 和 Linux 上有所不同：

- **在 Windows (CMD 或 PowerShell) 中激活：**
  ```powershell
  .\.venv\Scripts\activate
  ```

- **在 Linux 或 macOS (Bash/Zsh) 中激活：**
  ```bash
  source .venv/bin/activate
  ```

激活成功后，你的命令行提示符前面会出现 `(.venv)` 字样。

### 3. 安装项目依赖
请确保你的虚拟环境已激活。然后运行以下命令来安装所有必需的 Python 库：

```bash
pip install -r requirements.txt
```
这个过程会根据你的网络情况花费几分钟到十几分钟不等，请耐心等待。

### 4. 运行 NeoChat
所有依赖安装完成后，执行以下命令即可启动 NeoChat：

```bash
python main.py
```

### 5. 拉取最新的更新
当你想获取最新的开发版代码时，请在项目根目录（`NeoChat` 文件夹内）执行以下命令。

```bash
# 步骤一：放弃所有本地修改，避免冲突（注意：会丢失你的本地改动，请做好备份）
git reset --hard origin/main

# 步骤二：从 GitHub 拉取最新代码
git pull
```
**提示**：如果更新后 `requirements.txt` 文件发生了变化，你可能需要再次运行 `pip install -r requirements.txt` 来安装新增的依赖。

## 五、常见问题 (FAQ)

-   **Q: 输入 `git` 或 `python` 命令时，提示“不是内部或外部命令...”？**
    A: 这说明 Git 或 Python 没有被正确安装，或者安装时忘记勾选 "Add to PATH"。请回到【准备工作】章节，卸载后重新安装，**务必记得勾选 "Add to PATH" 选项**。

-   **Q: 运行 `python main.py` 时报错 `ModuleNotFoundError: No module named 'xxxx'`？**
    A: 这个错误说明缺少某个库。通常有两个原因：
    
    1.  你忘记激活虚拟环境了。请检查命令行提示符前面是否有 `(.venv)` 字样，如果没有，请根据你的系统执行激活命令。
    2.  缺少部分依赖。请先激活虚拟环境，然后执行 `pip install -r requirements.txt` 确保所有依赖都已安装。如果还是缺少某个特定的库 `xxxx`，可以尝试手动安装 `pip install xxxx`。
    
-   **Q: `git pull` 更新代码时提示错误或冲突 (conflict) 怎么办？**
    A: 作为测试者，你本地的代码一般不需要修改。如果遇到冲突，最简单的办法是放弃本地的所有改动，强制和服务器保持一致。在终端执行以下命令：
    
    ```powershell
    git reset --hard origin/main
    git pull
    ```
    **注意：这个命令会丢弃你可能在本地做的任何修改，请及时备份！** 对于只想体验最新版的用户来说，这是最直接有效的方法。

---

感谢你为 NeoChat 做出的贡献！如果你在使用过程中发现了任何 Bug 或者有好的建议，欢迎随时向我们提 [Issue](https://github.com/T-Auto/NeoChat/issues)！
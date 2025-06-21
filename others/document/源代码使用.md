# LingChat 开发版 Windows 环境配置与使用指南

LingChat 几乎每天都在更新，但是很长时间才会发布一个 release 版本。如果你想抢先使用新功能，或者想为 LingChat 项目做贡献，但是自己不会写代码，我们也欢迎你体验最新的开发版并及时向我们汇报 Bug。

本篇文档将手把手教你如何在 Windows 电脑上，从零开始配置环境，运行 LingChat 最新的开发版代码。即使你完全不懂编程。

欢迎你，勇于探索的测试者！

![c5845cf9148b2620b2740c40d73cc8ab](https://github.com/user-attachments/assets/2815cca5-e037-477e-8d18-c1eb385c5deb)

## 零、准备工作：安装必备工具

在开始之前，我们需要在你的电脑上安装三个免费的代码开发工具。

### 1. 安装 Git

Git 是一个代码版本管理工具，我们可以用它轻松地从 GitHub 上下载和更新 LingChat 的源代码。

- **下载地址**：[https://git-scm.com/download/win](https://git-scm.com/download/win)
- **安装方法**：下载后，双击打开安装包。你不需要理解每个选项的含义，**一路点击 "Next"** 使用默认设置完成安装即可。

### 2. 安装 Python

Python 是 LingChat 使用的编程语言。

- **下载地址**：[https://www.python.org/downloads/](https://www.python.org/downloads/)
- **推荐版本**：建议下载 3.12.10 版本，LingChat是基于python 3.12.10 开发的
- **安装方法**：
    1.  下载后，双击打开安装包。
    2.  **【非常重要！】** 在安装界面的最下方，**务必勾选 "Add Python to PATH"** 选项，然后再点击 "Install Now"。
    3.  等待安装完成即可。



### 3. 安装 VS Code

VS Code (Visual Studio Code) 是一个强大的代码编辑器，我们将用它来管理和运行 LingChat。

- **下载地址**：[https://code.visualstudio.com/](https://code.visualstudio.com/)
- **安装方法**：下载后，双击打开安装包。同样，使用默认设置，一路点击 "Next" 完成安装。

## 一、获取最新的源代码

准备工作完成后，我们开始获取 LingChat 的源代码。

1.  **创建项目文件夹**：在你的电脑上找一个合适的位置（比如 D 盘），创建一个新文件夹，专门用来存放 LingChat 的项目。例如，可以命名为 `MyProjects`。

2.  **打开命令行工具**：
    -   进入你刚刚创建的 `MyProjects` 文件夹。
    -   在文件夹窗口的地址栏里，输入 `cmd` 然后按回车键。这会弹出一个黑色的命令行窗口。

    

3.  **下载代码**：在弹出的黑色窗口中，复制并粘贴以下命令，然后按回车键。

    ```bash
    git clone -b develop https://github.com/SlimeBoyOwO/LingChat.git
    ```

    -   `git clone` 是下载命令。
    -   `-b develop` 表示我们要下载 `develop` 分支（也就是最新的开发版）。

    当你看到命令行提示完成，并且 `MyProjects` 文件夹下出现了一个名为 `LingChat` 的新文件夹时，就说明代码已经成功下载到你的电脑里了！

## 二、使用 VS Code 配置和运行项目

现在，我们将使用 VS Code 来完成最后的配置和运行。

1.  **用 VS Code 打开项目**：
    -   启动你刚刚安装的 VS Code。
    -   点击左上角的 "File" -> "Open Folder..." (或者 "文件" -> "打开文件夹...")。
    -   在弹出的窗口中，选择我们刚刚下载的 `LingChat` 文件夹，然后点击 "选择文件夹"。

2.  **打开 VS Code 的终端**：
    -   在 VS Code 的顶部菜单栏，点击 "Terminal" -> "New Terminal" (或者 "终端" -> "新建终端")。
    -   VS Code 的下方会弹出一个集成的命令行窗口，我们后续的命令都在这里输入。

    

### 2.1 虚拟环境的创建与维护

为了不污染你电脑本身的 Python 环境，我们将为 LingChat 创建一个独立的“虚拟环境”。这就像为这个项目准备了一个专属的、干净的工具箱。

1.  **创建虚拟环境**：在 VS Code 下方的终端里，输入以下命令并按回车。

    ```powershell
    python -m venv venv
    ```

    这个命令会创建一个名为 `venv` 的文件夹，里面存放着这个项目专用的 Python 环境。

2.  **激活虚拟环境**：输入以下命令并按回车，来“进入”这个专属工具箱。

    ```powershell
    .\venv\Scripts\activate
    ```

    成功激活后，你会看到命令行提示符的前面出现了 `(venv)` 的字样。这表示你已经处于虚拟环境中。

    

3.  **安装项目依赖**：LingChat 的运行需要很多第三方库的支持。我们用一条命令就能全部装好。

    ```powershell
    pip install -r requirements.txt
    ```

    这个过程可能会需要几分钟，请耐心等待它下载和安装。

    注意，如果出现 ERROR: Could not find a version that satisfies the requirement 等错误，可以关闭VPN重试

4.  **运行 LingChat！**：万事俱备！现在双击start.windows.bat，他可以依据你刚刚创建的虚拟环境来启动程序。

    如果一切顺利，你应该能看到 LingChat 的程序界面弹出来了。恭喜你，成功运行了最新的开发版！

### 2.2 如何同步最新的更新？

LingChat 的开发版更新非常快，当你想要体验最新的功能或修复时，只需要几个简单的步骤就可以同步更新。

1.  **打开项目并进入虚拟环境**：
    -   用 VS Code 打开 `LingChat` 文件夹。
    -   打开新的终端 (`Terminal` -> `New Terminal`)。
    -   激活虚拟环境：`.\venv\Scripts\activate`

2.  **拉取最新代码**：在终端中输入以下命令，从 GitHub 上拉取最新的代码。

    ```powershell
    git pull
    ```

3.  **更新依赖库（重要）**：有时候，新的代码会需要新的第三方库。所以，拉取代码后最好都执行一次更新依赖的命令。

    ```powershell
    pip install -r requirements.txt
    ```

    `pip` 很智能，它只会安装新增或有变动的库，不会重复安装已有的库。

4.  **重新运行程序**：

    现在双击start.windows.bat，你运行的就是包含所有最新改动的 LingChat 啦！

## 三、常见问题 (FAQ)

-   **Q: 输入 `git` 或 `python` 命令时，提示“不是内部或外部命令...”？**
    A: 这说明 Git 或 Python 没有被正确安装，或者安装时忘记勾选 "Add to PATH"。请回到【准备工作】章节，卸载后重新安装，**务必记得勾选 "Add to PATH" 选项**。

-   **Q: 运行 `python main.py` 时报错 `ModuleNotFoundError: No module named 'xxxx'`？**
    A: 这个错误说明缺少某个库。通常有两个原因：
    1.  你忘记激活虚拟环境了。请检查终端提示符前面是否有 `(venv)` 字样，如果没有，请执行 `.\venv\Scripts\activate`。
    2.  你忘记安装依赖了，或者更新代码后没有同步更新依赖。请在激活虚拟环境后，执行 `pip install -r requirements.txt`。

-   **Q: `git pull` 更新代码时提示错误或冲突 (conflict) 怎么办？**
    A: 作为测试者，你本地的代码一般不需要修改。如果遇到冲突，最简单的办法是放弃本地的所有改动，强制和服务器保持一致。在终端执行以下命令：
    ```powershell
    git reset --hard origin/develop
    git pull
    ```
    **注意：这个命令会丢弃你可能在本地做的任何修改。** 对于只想体验最新版的用户来说，这是最直接有效的方法。

---

感谢你为 LingChat 社区做出的贡献！如果你在使用过程中发现了任何 Bug 或者有好的建议，欢迎随时向我们提 [Issue](https://github.com/SlimeBoyOwO/LingChat/issues)！

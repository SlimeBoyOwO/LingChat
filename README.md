# 灵聊 简单的webui聊天程序！

> 通过python和nodejs和前端代码实现的简单对话功能。

## 支持操作系统：

Win10 以上，Win7经过测试无法运行！

## 功能列表

- ✅ 随时和用户对话，可以使用表情，动作和聊天气泡。
- ✅ 在logs中记录你们聊天的记录。
- ✅ 在菜单更改设置并且浏览当前历史记录。

## 如何使用？

1，下载并安装python3.10，**※※安装时请勾选Add python.exe to PATH※※**

<img src="https://s1.imagehub.cc/images/2025/04/15/bf275367a931767a4636940e2a2dca75.png" alt="Python" style="zoom:50%;" />

- [64位版本](https://www.python.org/ftp/python/3.10.11/python-3.10.11-amd64.exe)
- [32位版本](https://www.python.org/ftp/python/3.10.11/python-3.10.11.exe)

2，下载并安装Node.js

- [64位版本](https://nodejs.org/dist/v22.14.0/node-v22.14.0-x64.msi)
- [32位版本](https://nodejs.org/dist/v22.14.0/node-v22.14.0-x86.msi)

3，从网盘**下载情感分类模型**，放在backend/emotion_model_12emo中。

- [百度网盘](https://pan.baidu.com/s/16Dy53KX3jIjACY5fCctKDA)：请在这里下载emotion_model_12emo，提取码：0721
- [123云盘](https://www.123865.com/s/7YDfjv-KRK5v): 如果你没有百度网盘会员，请从此处下载emotion_model_12emom
- [Google云盘](https://drive.google.com/file/d/1LWdJYYc3QaYbzHupt5DDaM1lCeG-X5vd/view?usp=sharing): 如果你是非大陆或者海外朋友，下载这个

4，在[backend/predictor.py](https://github.com/SlimeBoyOwO/LingChat/blob/main/backend/predictor.py)里填写你的deepseek apikey，deepseek apikey登录[DeepSeek 开放平台](https://platform.deepseek.com/usage)后获取。请妥善保管自己的apikey。

5，双击[start.bat](https://github.com/SlimeBoyOwO/LingChat/blob/main/start.bat)一键启动。初次启动需要确保网络通畅，并耐心等待约20分钟。若中途网络问题导致安装丢包，请手动删除.venv并再次双击。

6，若要使用语音功能，请下载[simple-vits-api](https://github.com/Artrajz/vits-simple-api)链接程序。该项目实现了基于 VITS 的简单语音合成 API。建议下载GPU版本，速度快。程序默认监听23456语音端口，程序默认导入的模型是zcchat地址->讨论区->角色示范（丛雨）->vits模型下载好之后在simple-vits-api的目录的/data/models里面解压，再启动就ok了

_※目前已知问题：若电脑配置较低，python后端启动较慢，需要等待命令行窗口显示后端成功链接后，刷新浏览器_

_※出现其他报错请截图反馈_



### 若一键包出现BUG，可采用备选方案：

```markdown
1. 下载好仓库内的东西，确保你下载了nodejs和python环境
2. 点击backend/install.bat安装必要的库（该库是全局安装的！！如果你电脑已经有python环境了谨慎操作）
3. 从网盘下载情感分类模型，放在backend/emotion_model_12emo中
4. 由于隐私原因，请在deepseek.py输入自己的api或者自己改写程序接入其他api
5. 启动backend中的run.bat，启动根目录的run_server.bat即可启动
6. 输入localhost:3000 进入聊天界面，左上角显示已连接服务器则表示完成
7. 为了使用语音功能，请前往链接下载vits链接程序！程序默认监听23456语音端口
8. 程序默认导入的模型是zcchat地址->讨论区->角色示范（丛雨）->vits模型下载好之后在simple-vits-api的目录的/data/models里面解压，再启动就ok了
9. 如果需要使用其他模型，在webChat.py的Vits实现函数更改相关设定即可
```

## 相关设定

1. 在deepseek.py里的settings设定角色性格和你的设定（别忘了自己的API一定要填写）
2. 可以更换/public/pictures/lingling/里面的立绘+修改/public/css/galgame.css里的代码实现自定义角色或表情动作气泡
3. /public/js/talk.js 里面可以设定不同的心情和不同的动作，目前有12种情绪，由于模型是自己训练的所以更新要等一段时间啦

## 相关链接

- [emotion_model_12emo 百度网盘](https://pan.baidu.com/s/16Dy53KX3jIjACY5fCctKDA)：请在这里下载emotion_model_12emo，提取码：0721
- [emotion_model_12emo 123云盘](https://www.123865.com/s/7YDfjv-KRK5v): 或这里下载emotion_model_12emom更快一点如果你没有百度网盘会员
- [emotion_model_12emo Google云盘](https://drive.google.com/file/d/1LWdJYYc3QaYbzHupt5DDaM1lCeG-X5vd/view?usp=sharing): 如果你是非大陆或者海外朋友，下载这个
- [simple-vits-api](https://github.com/Artrajz/vits-simple-api): 该项目实现了基于 VITS 的简单语音合成 API。建议下载GPU版本，速度快
- [zcchat](https://github.com/Zao-chen/ZcChat): 本项目的灵感来源，可以在这里找到Vits模型和人物素材

## 一些小话

- 本项目为了快速开发用了很多AI工具，有做的不好的地方欢迎指出！
- 会随着项目的知名度提供更便利清晰的自定义功能的！目前实在没时间啦...
- 本项目更多作为一个超小型的学习项目，由于文件结构非常简单，欢迎有兴趣的人学习。

## For 开发者：

### 系统依赖

#### python 依赖

- 本项目基于python-3.10开发，经测试也兼容python-3.11和python-3.12，不兼容python-3.13

### 在 Linux 服务器上部署 LingChat 项目

1. **克隆项目仓库**

   ```bash
   git clone https://github.com/SlimeBoyOwO/LingChat.git
   ```

   这会将项目代码下载到当前目录下的 `LingChat` 文件夹中。

2. **进入项目目录**

   ```bash
   cd LingChat
   ```

   后续所有命令都在这个 `LingChat` 目录下执行。

3. **创建 Python 虚拟环境**

   ```bash
   python3 -m venv .venv
   ```

   这会在当前目录下创建一个名为 `.venv` 的文件夹，包含独立的 Python 环境。

4. **激活 Python 虚拟环境**

   ```bash
   source .venv/bin/activate
   ```

5. **安装 Python 依赖**

   ```bash
   pip install -r requirements.txt
   ```

6. **安装 Node.js 依赖 (前端需要)**

   ```bash
   npm install
   ```

7. **运行后端服务 (webChat.py)**
   *你需要让这个进程在后台运行，否则它会占用你的终端。这里提供几种常见方法：*

   * **方法一：使用 `nohup` 和 `&` (简单)**

     ```bash
     nohup python backend/webChat.py &
     ```

   * **方法二：使用 `screen` 或 `tmux` (推荐，更易管理)**
     如果你熟悉 `screen` 或 `tmux`，可以创建一个会话来运行后端：

     ```bash
     # (可选) 安装 screen: sudo apt install screen 或 sudo yum install screen
     screen -S backend_lingchat # 创建并进入名为 backend_lingchat 的 screen 会话
     python backend/webChat.py # 运行后端
     ```

8. **运行前端服务 (server.js)**
   *同样，前端服务也需要后台运行。*

   * **方法一：使用 `nohup` 和 `&`**

     ```bash
     nohup node server.js &
     ```

   * **方法二：使用 `screen` 或 `tmux`**
     可以在另一个 `screen` 或 `tmux` 会话中运行前端：

     ```bash
     screen -S frontend_lingchat # 创建并进入前端会话
     node server.js # 运行前端
     ```

   * **方法三：使用 `pm2` (专门管理 Node.js 应用，也可管理 Python)**
     如果你希望更专业地管理 Node.js (甚至 Python) 进程，`pm2` 是个很好的选择：

     ```bash
     source .venv/bin/activate 
     pm2 start backend/webChat.py --name lingchat-backend --interpreter python3 
     pm2 start server.js --name lingchat-frontend
     ```
     
### 在Winows环境部署该项目

1. **克隆项目仓库:**

   ```bash
   git clone https://github.com/SlimeBoyOwO/LingChat.git
   ```

2. **进入项目目录:**

   ```bash
   cd LingChat
   ```

3. **创建 Python 虚拟环境:**

   ```bash
   python -m venv .venv
   ```

4. **激活 Python 虚拟环境:**

   * **在 cmd.exe 中:**

     ```bash
     .venv\Scripts\activate
     ```

   * **在 PowerShell 中:**

     ```powershell
     .venv\Scripts\Activate.ps1
     ```

5. **安装 Python 依赖:** (确保虚拟环境已激活，提示符前有 `(.venv)`)

   ```bash
   pip install -r requirements.txt
   ```

6. **安装 Node.js 依赖:** (不需要激活 Python 虚拟环境，可以在项目根目录直接执行)

   ```bash
   npm install
   ```

7. **运行后端服务 (webChat.py) 和 前端服务 (server.js):**

   * **运行后端:**

     ```bash
     python backend/webChat.py
     ```

   * **运行前端:**

     ```bash
     node server.js
     ```

8. 安装 vits-simple-api

```bash
git submodule init
git submodule update
cd third_party/vits-simple-api
python -m pip install gunicorn
# for windows:
# pip install eunjeon  -i https://pypi.artrajz.cn/simple/
python -m pip install -r requirements.txt
```


## 其他

> 本项目使用的气泡+音效素材来源于碧蓝档案，请勿商用
> 默认简单狼狼立绘是自绘，表情差分源于AI，如果你想自己创作可使用 Novelai 网站
> 有其他问题可以B站私信捏

© 诺一 钦灵

# LingChat 带动作与情感检测的人工智能对话GalGame
![454545644](https://github.com/user-attachments/assets/509269df-afcc-465e-8333-0dec9eb8e89e)
## 支持操作系统：

Win10 以上，linux 请用 docker 部署，Win7 目前待测试 目前 0.2.0 版本是测试版，0.1.0 稳定但缺少游戏存读档，永久记忆，18 情绪和多最新版本功能，请自行选择

## 使用相关说明

- 0.2.0 版本还处于测试版本，请在使用的时候注意以下几点，本版本 RAG 功能需要你能**挂梯子**，下版本就不需要啦，大学生忙着考试呢（）：
- 在.env 中有这么一行配置： USE_RAG=false # 是否启用 RAG 系统，设置为 true 或 false
- 若此项开启，说明你的每一次对话都会作为永久记忆的一部分，请**不要在开启这个 RAG 模式的情况下使用存档功能，否则可能出现未知严重 BUG**
- 如果出现语音问题（如重复播放老版本语音），请换个浏览器输入 localhost:8765 进入页面解决，此 bug 已修复但尚未发布修复版本，你可以下载使用 develop 的开发版本。
- 假如说你的电脑不是独显，而是核显，务必下载CPU版本的simple vits api!!!

## 功能列表

- ✅ 使用 18 种差分表情立绘，动作，音效，聊天气泡及语音与用户对话
- ✅ 在菜单更改设置并且导入聊天历史记录。
- ✅ 支持跨越对话的永久记忆，几乎不增加 Token 消耗。
- ✅ 支持使用 deepseek、标准 OpenAI 接口及 ollama 本地模型

## 如何使用？

### 下载&使用 exe 程序

- 在[Release](https://github.com/SlimeBoyOwO/LingChat/releases)中下载附件，并解压。
- 解压后，使用记事本打开 app 文件夹.env，在.env 中填入你的 apikey。deepseek apikey 登录[DeepSeek 开放平台](https://platform.deepseek.com/usage)后获取。请妥善保管自己的 apikey。
- 点击 LingChat.exe 启动程序
- (非必须):若要使用语音功能，请下载[simple-vits-api](https://github.com/Artrajz/vits-simple-api)链接程序。该项目实现了基于 VITS 的简单语音合成 API。建议下载 GPU 版本，速度快。程序默认监听 23456 语音端口，程序默认导入的模型是 zcchat 地址->讨论区->角色示范（丛雨）->vits 模型下载好之后在 simple-vits-api 的目录的/data/models 里面解压，再启动就 ok 了;如果需要使用其他模型，在.env 的 Vits 实现函数更改相关设定即可。
- app 文件夹内的 rag_chat_history 文件夹的所有对话记忆将被永久储存。打开 RAG 开关后，本轮对话将会储存在 rag_chat_history 文件夹内。**如果你手动更改了该文件夹内部的对话记录，请手动删除 app 文件夹下的整个 chroma_db_store 文件夹以更新记忆库**。该文件夹是提高启动速度的永久记忆缓存区域。

### 下载&使用最新的开发测试版本

- LingChat 几乎每天都在更新，但是很长时间才会发布一个 release 版本。如果你想抢先使用新功能，或者想为 LingChat 项目做贡献，但是自己不会写代码，我们也欢迎你体验最新的开发版并及时向我们汇报 Bug。
- 关于如何使用 develop 的源代码，可以参照[使用教程](https://github.com/SlimeBoyOwO/LingChat/blob/develop/others/document/%E6%BA%90%E4%BB%A3%E7%A0%81%E4%BD%BF%E7%94%A8.md)
- LingChat现已支持长线预设剧情模式，你可以参照[剧情创作指南](https://github.com/SlimeBoyOwO/LingChat/blob/main/others/document/AI-Galgame%20%E5%89%A7%E6%83%85%E5%88%9B%E4%BD%9C%E6%8C%87%E5%8D%97)撰写属于你的Galgame！

### 下载情感分类模型

情感分类模型已包含在 Releases 中，双击 exe 即可启动。源代码内不包含，请手动下载 release 然后移动过去。

## 相关设定（For 开发者们）

1. backend 是 Python 的后端程序，frontend 是 html javascript css 的前端文件；start.bat 默认使用 python3.12 创建.venv 虚拟环境
2. 可以更换/public/pictures/lingling/里面的立绘+修改/public/css/galgame.css 里的代码实现自定义角色或表情动作气泡
3. /public/js/emotion 里面可以设定不同的心情和不同的动作，目前有 18 种情绪，更多情绪和模型优化还在更新
4. **main 只会发布稳定版，使用最新功能请切换至 develop 分支**

## 相关链接

- [simple-vits-api](https://github.com/Artrajz/vits-simple-api): 该项目实现了基于 VITS 的简单语音合成 API。建议下载 GPU 版本，速度快
- [zcchat](https://github.com/Zao-chen/ZcChat): 本项目的灵感来源，可以在这里找到 Vits 模型和人物素材

## 一些小话

- 本项目为了快速开发用了很多 AI 工具，有做的不好的地方欢迎指出！
- 感谢一路结识的开发者，都是香软可爱又厉害的大佬们~ 如果你有开发意向可以联系我！
- 本项目更多作为一个超小型的学习项目，由于文件结构非常简单，欢迎有兴趣的人学习。

## 其他

> 本项目使用的气泡+音效素材来源于碧蓝档案，请勿商用  
> 默认简单狼狼立绘是自绘，表情差分源于 AI，如果你想自己创作可使用 Novelai 网站  
> 有其他问题可以 B 站私信捏

© 诺一 钦灵

## 支持操作系统：

Win10 以上，linux请用docker部署，Win7目前待测试 目前0.2.0版本是测试版，0.1.0稳定但缺少游戏存读档，永久记忆，18情绪和多最新版本功能，请自行选择

## 功能列表

- ✅ 使用18种差分表情立绘，动作，音效，聊天气泡及语音与用户对话
- ✅ 在菜单更改设置并且导入聊天历史记录。
- ✅ 支持跨越对话的永久记忆，几乎不增加Token消耗。
- ✅ 支持使用deepseek、标准OpenAI接口及ollama本地模型

## 如何使用？

### 下载&使用exe程序

- 在[Release](https://github.com/SlimeBoyOwO/LingChat/releases)中下载附件，并解压。
- 解压后，使用记事本打开app文件夹.env，在.env中填入你的apikey。deepseek apikey登录[DeepSeek 开放平台](https://platform.deepseek.com/usage)后获取。请妥善保管自己的apikey。
- 点击LingChat.exe启动程序
- (非必须):若要使用语音功能，请下载[simple-vits-api](https://github.com/Artrajz/vits-simple-api)链接程序。该项目实现了基于 VITS 的简单语音合成 API。建议下载GPU版本，速度快。程序默认监听23456语音端口，程序默认导入的模型是zcchat地址->讨论区->角色示范（丛雨）->vits模型下载好之后在simple-vits-api的目录的/data/models里面解压，再启动就ok了;如果需要使用其他模型，在.env的Vits实现函数更改相关设定即可。
- app文件夹内的rag_chat_history文件夹的所有对话记忆将被永久储存。打开RAG开关后，本轮对话将会储存在rag_chat_history文件夹内。**如果你手动更改了该文件夹内部的对话记录，请手动删除app文件夹下的整个chroma_db_store文件夹以更新记忆库**。该文件夹是提高启动速度的永久记忆缓存区域。

### 下载情感分类模型
情感分类模型已包含在Releases中，双击exe即可启动。源代码内不包含，请手动下载release然后移动过去。

## 使用相关说明
- 0.2.0 版本还处于测试版本，请在使用的时候注意以下几点：
- 在.env中有这么一行配置：  USE_RAG=false           # 是否启用RAG系统，设置为true或false
- 若此项开启，说明你的每一次对话都会作为永久记忆的一部分，请**不要在开启这个RAG模式的情况下使用存档功能，否则可能出现未知严重BUG**

## 相关设定（For 开发者们）

1. backend是Python的后端程序，frontend是html javascript css的前端文件；start.bat默认使用python3.12创建.venv虚拟环境
2. 可以更换/public/pictures/lingling/里面的立绘+修改/public/css/galgame.css里的代码实现自定义角色或表情动作气泡
3. /public/js/emotion 里面可以设定不同的心情和不同的动作，目前有18种情绪，更多情绪和模型优化还在更新
4. **main只会发布稳定版，使用最新功能请切换至develop分支**

## 相关链接

- [simple-vits-api](https://github.com/Artrajz/vits-simple-api): 该项目实现了基于 VITS 的简单语音合成 API。建议下载GPU版本，速度快
- [zcchat](https://github.com/Zao-chen/ZcChat): 本项目的灵感来源，可以在这里找到Vits模型和人物素材

## 一些小话

- 本项目为了快速开发用了很多AI工具，有做的不好的地方欢迎指出！
- 感谢一路结识的开发者，都是香软可爱又厉害的大佬们~ 如果你有开发意向可以联系我！
- 本项目更多作为一个超小型的学习项目，由于文件结构非常简单，欢迎有兴趣的人学习。

## 其他

> 本项目使用的气泡+音效素材来源于碧蓝档案，请勿商用  
> 默认简单狼狼立绘是自绘，表情差分源于AI，如果你想自己创作可使用 Novelai 网站  
> 有其他问题可以B站私信捏

© 诺一 钦灵

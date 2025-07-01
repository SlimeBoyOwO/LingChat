## 支持操作系统：

Win10 以上，linux 请用 docker 部署，Win7 经过测试无法运行！【在即将发布的 0.2.0 版本，理论上可以运行在 win7】

## 功能列表

- ✅ 随时和用户对话，可以使用表情，动作和聊天气泡。
- ✅ 在 logs 中记录你们聊天的记录。
- ✅ 在菜单更改设置并且浏览当前历史记录。

## 我们的下个版本：

- 目前在 develop 分支更新，马上就会发布了，敬请期待~

## 如何使用？

### 下载&使用 exe 程序

- 在[v0.1.0-beta 发布页](https://github.com/SlimeBoyOwO/LingChat/releases/tag/v0.1.0-beta)中下载[LingChat.zip](https://github.com/SlimeBoyOwO/LingChat/releases/download/v0.1.0-beta/LingChat.zip)，并解压。
- 下载并安装 Node.js | [64 位版本](https://nodejs.org/dist/v22.14.0/node-v22.14.0-x64.msi) | [32 位版本](https://nodejs.org/dist/v22.14.0/node-v22.14.0-x86.msi) |
- 使用记事本打开.env，在.env 中填入你的 apikey。deepseek apikey 登录[DeepSeek 开放平台](https://platform.deepseek.com/usage)后获取。请妥善保管自己的 apikey。
- 点击 LingChat.exe 启动程序

- **重要**: 最近我们移除了 node_modules，如果你启动时候出现了 Error: Cannot find module 'express' 这种错误，请 WIN+R 并 CD 到当前目录并且输入 npm install express 或其他任何在'xx'内的模块！

- (非必须):若要使用语音功能，请下载[simple-vits-api](https://github.com/Artrajz/vits-simple-api)链接程序。该项目实现了基于 VITS 的简单语音合成 API。建议下载 GPU 版本，速度快。程序默认监听 23456 语音端口，程序默认导入的模型是 zcchat 地址->讨论区->角色示范（丛雨）->vits 模型下载好之后在 simple-vits-api 的目录的/data/models 里面解压，再启动就 ok 了;如果需要使用其他模型，在.env 的 Vits 实现函数更改相关设定即可。

### 下载情感分类模型

情感分类模型已包含在 Releases 中，双击 exe 即可启动。但源代码中不包含模型，若你想使用源代码，需要单独下载：

- [emotion_model_12emo 百度网盘](https://pan.baidu.com/s/16Dy53KX3jIjACY5fCctKDA)：请在这里下载 emotion_model_12emo，提取码：0721
- [emotion_model_12emo 123 云盘](https://www.123865.com/s/7YDfjv-KRK5v): 或这里下载 emotion_model_12emom 更快一点如果你没有百度网盘会员
- [emotion_model_12emo Google 云盘](https://drive.google.com/file/d/1LWdJYYc3QaYbzHupt5DDaM1lCeG-X5vd/view?usp=sharing): 如果你是非大陆或者海外朋友，下载这个

## 相关设定（For 开发者们）

1. backend 是 Python 的后端程序，frontend 是 nodejs html javascript css 的前端文件；start.bat 默认使用 python3.12 创建.venv 虚拟环境
2. 可以更换/public/pictures/lingling/里面的立绘+修改/public/css/galgame.css 里的代码实现自定义角色或表情动作气泡
3. /public/js/talk.js 里面可以设定不同的心情和不同的动作，目前有 12 种情绪，由于模型是自己训练的所以更新要等一段时间啦
4. main 只会发布稳定版，使用最新功能请切换至 develop 分支

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

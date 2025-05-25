## 支持操作系统：

Win10 以上，linux请用docker部署，Win7目前待测试

## 功能列表

- ✅ 使用18种差分表情立绘，动作，音效，聊天气泡及语音与用户对话
- ✅ 在菜单更改设置并且导入聊天历史记录。
- ✅ 支持跨越对话的永久记忆，几乎不增加Token消耗
- ✅ 支持使用deepseek、标准OpenAI接口及ollama本地模型

## 如何使用？

### 下载&使用exe程序

- 在[Release](https://github.com/SlimeBoyOwO/LingChat/releases)中下载附件，并解压。
- 解压后，使用记事本打开app文件夹.env，在.env中填入你的apikey。deepseek apikey登录[DeepSeek 开放平台](https://platform.deepseek.com/usage)后获取。请妥善保管自己的apikey。
- 点击LingChat.exe启动程序

- **重要**: 最近我们移除了node_modules，如果你启动时候出现了 Error: Cannot find module 'express' 这种错误，请WIN+R并CD到当前目录并且输入npm install express或其他任何在'xx'内的模块！

- (非必须):若要使用语音功能，请下载[simple-vits-api](https://github.com/Artrajz/vits-simple-api)链接程序。该项目实现了基于 VITS 的简单语音合成 API。建议下载GPU版本，速度快。程序默认监听23456语音端口，程序默认导入的模型是zcchat地址->讨论区->角色示范（丛雨）->vits模型下载好之后在simple-vits-api的目录的/data/models里面解压，再启动就ok了;如果需要使用其他模型，在.env的Vits实现函数更改相关设定即可。

### 下载情感分类模型
情感分类模型已包含在Releases中，双击exe即可启动。但源代码中不包含模型，若你想使用源代码，需要单独下载：
- [emotion_model_12emo 百度网盘](https://pan.baidu.com/s/16Dy53KX3jIjACY5fCctKDA)：请在这里下载emotion_model_12emo，提取码：0721
- [emotion_model_12emo 123云盘](https://www.123865.com/s/7YDfjv-KRK5v): 或这里下载emotion_model_12emom更快一点如果你没有百度网盘会员
- [emotion_model_12emo Google云盘](https://drive.google.com/file/d/1LWdJYYc3QaYbzHupt5DDaM1lCeG-X5vd/view?usp=sharing): 如果你是非大陆或者海外朋友，下载这个

## 相关设定（For 开发者们）

1. backend是Python的后端程序，frontend是nodejs html javascript css的前端文件；start.bat默认使用python3.12创建.venv虚拟环境
2. 可以更换/public/pictures/lingling/里面的立绘+修改/public/css/galgame.css里的代码实现自定义角色或表情动作气泡
3. /public/js/talk.js 里面可以设定不同的心情和不同的动作，目前有12种情绪，由于模型是自己训练的所以更新要等一段时间啦
4. main只会发布稳定版，使用最新功能请切换至develop分支

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

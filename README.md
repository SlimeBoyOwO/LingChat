## 支持操作系统：

Win10 以上，Win7经过测试无法运行！

## 功能列表

- ✅ 随时和用户对话，可以使用表情，动作和聊天气泡。
- ✅ 在logs中记录你们聊天的记录。
- ✅ 在菜单更改设置并且浏览当前历史记录。

## 如何使用？

### 使用测试版本

- 在[v0.1.0-beta发布页](https://github.com/SlimeBoyOwO/LingChat/releases/tag/v0.1.0-beta)中下载[LingChat.zip](https://github.com/SlimeBoyOwO/LingChat/releases/download/v0.1.0-beta/LingChat.zip)，并解压。
- 下载并安装Node.js | [64位版本](https://nodejs.org/dist/v22.14.0/node-v22.14.0-x64.msi) | [32位版本](https://nodejs.org/dist/v22.14.0/node-v22.14.0-x86.msi) |
- 使用记事本打开.env，在.env中填入你的apikey。deepseek apikey登录[DeepSeek 开放平台](https://platform.deepseek.com/usage)后获取。请妥善保管自己的apikey。
- 点击LingChat.exe启动程序

- (非必须):若要使用语音功能，请下载[simple-vits-api](https://github.com/Artrajz/vits-simple-api)链接程序。该项目实现了基于 VITS 的简单语音合成 API。建议下载GPU版本，速度快。程序默认监听23456语音端口，程序默认导入的模型是zcchat地址->讨论区->角色示范（丛雨）->vits模型下载好之后在simple-vits-api的目录的/data/models里面解压，再启动就ok了;如果需要使用其他模型，在webChat.py的Vits实现函数更改相关设定即可。

_※目前已知问题：若电脑配置较低，python后端启动较慢，需要等待命令行窗口显示后端成功链接后，刷新浏览器_

_※出现其他报错请截图反馈_

### **下载情感分类模型（exe用户不需要这个）**（）

- [emotion_model_12emo 百度网盘](https://pan.baidu.com/s/16Dy53KX3jIjACY5fCctKDA)：请在这里下载emotion_model_12emo，提取码：0721
- [emotion_model_12emo 123云盘](https://www.123865.com/s/7YDfjv-KRK5v): 或这里下载emotion_model_12emom更快一点如果你没有百度网盘会员
- [emotion_model_12emo Google云盘](https://drive.google.com/file/d/1LWdJYYc3QaYbzHupt5DDaM1lCeG-X5vd/view?usp=sharing): 如果你是非大陆或者海外朋友，下载这个

## 相关设定（For 开发者们）

1. backend是Python的后端程序，frontend是nodejs html javascript css的前端文件
2. 可以更换/public/pictures/lingling/里面的立绘+修改/public/css/galgame.css里的代码实现自定义角色或表情动作气泡
3. /public/js/talk.js 里面可以设定不同的心情和不同的动作，目前有12种情绪，由于模型是自己训练的所以更新要等一段时间啦

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
